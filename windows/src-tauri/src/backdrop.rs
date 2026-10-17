// Real frosted glass on Windows 11 (round 3C item 1): a Windows.UI.Composition
// visual tree under the island's webview, with the host backdrop brush (what
// is behind the window, blurred the way Windows 11's own flyouts are) clipped
// to a rounded rectangle that is the island's shape. The page draws its tint,
// rim and highlight on top, on its transparent background. The clip is the
// shape, so the blur can never spill into the corners whatever the window
// region does; the region is left for hit-testing.
//
// Composition lives on the main thread (it needs a DispatcherQueue there; the
// main thread's message loop serves it). Everything else hands it work with
// run_on_main_thread. Where it can't be made (Windows 10, an error), glass.rs
// falls back to the accent blur, then to an opaque-enough tint.

use std::sync::atomic::{AtomicU8, Ordering};

use tauri::AppHandle;

/// 0: not tried, 1: on, 2: not available.
static STATE: AtomicU8 = AtomicU8::new(0);

pub fn active() -> bool {
    STATE.load(Ordering::Relaxed) == 1
}

/// Makes the visual tree, on the main thread (call it from setup). True when it is on.
pub fn start(app: &AppHandle) -> bool {
    if STATE.load(Ordering::Relaxed) != 0 {
        return active();
    }
    let on = os::start(app);
    STATE.store(if on { 1 } else { 2 }, Ordering::Relaxed);
    crate::log::line(if on { "glass: host backdrop on (composition)" } else { "glass: no host backdrop here" });
    on
}

/// The island's shape in physical pixels: left, top, width, height, the bottom corners' radius.
pub fn shape(app: &AppHandle, rect: (f32, f32, f32, f32, f32)) {
    if !active() {
        return;
    }
    let _ = app.run_on_main_thread(move || os::shape(rect));
}

/// Shown while the island is glass; hidden for Solid.
pub fn show(app: &AppHandle, on: bool) {
    if !active() {
        return;
    }
    let _ = app.run_on_main_thread(move || os::show(on));
}

#[cfg(windows)]
mod os {
    use std::cell::RefCell;
    use std::time::Duration;

    use tauri::{AppHandle, Manager};
    use windows::core::{Interface, HSTRING};
    use windows_numerics::Vector2;
    use windows::Foundation::TimeSpan;
    use windows::System::DispatcherQueueController;
    use windows::UI::Composition::Desktop::DesktopWindowTarget;
    use windows::UI::Composition::{Compositor, CompositionRoundedRectangleGeometry, ContainerVisual};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_HOSTBACKDROPBRUSH};
    use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
    use windows::Win32::System::WinRT::{CreateDispatcherQueueController, DispatcherQueueOptions, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT};

    struct Tree {
        _queue: DispatcherQueueController,
        _compositor: Compositor,
        _target: DesktopWindowTarget,
        root: ContainerVisual,
        clip: CompositionRoundedRectangleGeometry,
    }

    thread_local! {
        static TREE: RefCell<Option<Tree>> = const { RefCell::new(None) };
    }

    /// How long the clip takes to follow a new shape: the island's unfold (round 3B item 8).
    const FOLLOW: Duration = Duration::from_millis(290);

    pub fn start(app: &AppHandle) -> bool {
        match build(app) {
            Ok(tree) => {
                TREE.with(|t| *t.borrow_mut() = Some(tree));
                true
            }
            Err(e) => {
                crate::log::line(format!("glass: composition backdrop failed: {e}"));
                false
            }
        }
    }

    fn build(app: &AppHandle) -> windows::core::Result<Tree> {
        let win = app.get_webview_window(crate::island::WINDOW_LABEL).ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(-1)))?;
        let hwnd = HWND(win.hwnd().map_err(|_| windows::core::Error::from(windows::core::HRESULT(-1)))?.0);
        unsafe {
            // Windows 11 draws the host backdrop for windows that ask for it.
            let on: windows::core::BOOL = true.into();
            DwmSetWindowAttribute(hwnd, DWMWA_USE_HOSTBACKDROPBRUSH, &on as *const _ as *const _, std::mem::size_of_val(&on) as u32)?;
            let queue = CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            })?;
            let compositor = Compositor::new()?;
            let interop: ICompositorDesktopInterop = compositor.cast()?;
            // Not topmost: under the webview, which draws the tint on a transparent background.
            let target = interop.CreateDesktopWindowTarget(hwnd, false)?;
            let root = compositor.CreateContainerVisual()?;
            root.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            let clip = compositor.CreateRoundedRectangleGeometry()?;
            clip.SetSize(Vector2 { X: 0.0, Y: 0.0 })?;
            root.SetClip(&compositor.CreateGeometricClipWithGeometry(&clip)?)?;
            let blur = compositor.CreateSpriteVisual()?;
            blur.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            blur.SetBrush(&compositor.CreateHostBackdropBrush()?)?;
            root.Children()?.InsertAtTop(&blur)?;
            // The clip follows the island with the same timing as its unfold.
            let follow = compositor.CreateImplicitAnimationCollection()?;
            for property in ["Size", "Offset"] {
                let anim = compositor.CreateVector2KeyFrameAnimation()?;
                anim.SetTarget(&HSTRING::from(property))?;
                anim.InsertExpressionKeyFrame(1.0, &HSTRING::from("this.FinalValue"))?;
                anim.SetDuration(TimeSpan { Duration: FOLLOW.as_nanos() as i64 / 100 })?;
                follow.Insert(&HSTRING::from(property), &anim)?;
            }
            clip.SetImplicitAnimations(&follow)?;
            target.SetRoot(&root)?;
            Ok(Tree { _queue: queue, _compositor: compositor, _target: target, root, clip })
        }
    }

    pub fn shape(rect: (f32, f32, f32, f32, f32)) {
        let (x, y, w, h, r) = rect;
        TREE.with(|t| {
            if let Some(tree) = t.borrow().as_ref() {
                // Rounded all round, but starting one radius above the top:
                // the island hangs from the screen's edge with square top corners.
                let _ = tree.clip.SetCornerRadius(Vector2 { X: r, Y: r });
                let _ = tree.clip.SetOffset(Vector2 { X: x, Y: y - r });
                let _ = tree.clip.SetSize(Vector2 { X: w.max(0.0), Y: (h + r).max(0.0) });
            }
        });
    }

    pub fn show(on: bool) {
        TREE.with(|t| {
            if let Some(tree) = t.borrow().as_ref() {
                let _ = tree.root.SetIsVisible(on);
            }
        });
    }
}

#[cfg(not(windows))]
mod os {
    pub fn start(_app: &tauri::AppHandle) -> bool {
        false
    }
    pub fn shape(_rect: (f32, f32, f32, f32, f32)) {}
    pub fn show(_on: bool) {}
}
