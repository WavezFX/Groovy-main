//! Round 3G item 2: a window may call only what its capability allows
//! (capabilities/<window>.json). The commands are called through Tauri's own
//! IPC on the mock runtime, with Groovy's real configuration and ACL. The ACL
//! goes by the command's name, so stand-ins with Groovy's names check the real
//! capabilities without running anything.

use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::Manager;

#[tauri::command]
fn studio_run() {}
#[tauri::command]
fn studio_write() {}
#[tauri::command]
fn studio_workspaces() {}
#[tauri::command]
fn tools_set_permission() {}
#[tauri::command]
fn tools_decide() {}
#[tauri::command]
fn local_set_key() {}
#[tauri::command]
fn notify_error() {}

fn call(app: &tauri::App<MockRuntime>, window: &str, cmd: &str) -> Result<(), String> {
    let w = app.get_webview_window(window).unwrap_or_else(|| tauri::WebviewWindowBuilder::new(app, window, Default::default()).build().unwrap());
    let url = if cfg!(windows) { "http://tauri.localhost" } else { "tauri://localhost" };
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: url.parse().unwrap(),
        body: InvokeBody::Json(serde_json::json!({})),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(&w, request).map(|_| ()).map_err(|e| e.to_string())
}

fn refused(r: &Result<(), String>) -> bool {
    r.as_ref().err().is_some_and(|e| e.contains("not allowed"))
}

#[test]
fn each_window_is_refused_what_it_has_no_use_for() {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![studio_run, studio_write, studio_workspaces, tools_set_permission, tools_decide, local_set_key, notify_error])
        .build(tauri::generate_context!())
        .unwrap();
    // Running and writing: the Studio only.
    for window in ["island", "settings", "mochi"] {
        for cmd in ["studio_run", "studio_write", "studio_workspaces"] {
            assert!(refused(&call(&app, window, cmd)), "{window} may call {cmd}");
        }
    }
    // Keys and permissions: Settings only.
    for window in ["island", "mochi", "studio"] {
        assert!(refused(&call(&app, window, "local_set_key")), "{window} may set a key");
        assert!(refused(&call(&app, window, "tools_set_permission")), "{window} may change a permission");
    }
    assert!(refused(&call(&app, "mochi", "tools_decide")));
    assert!(refused(&call(&app, "studio", "tools_decide")));
    // What each one uses still goes through.
    assert_eq!(call(&app, "studio", "studio_run"), Ok(()));
    assert_eq!(call(&app, "settings", "tools_set_permission"), Ok(()));
    assert_eq!(call(&app, "settings", "local_set_key"), Ok(()));
    assert_eq!(call(&app, "island", "tools_decide"), Ok(()));
    for window in ["island", "settings", "mochi", "studio"] {
        assert_eq!(call(&app, window, "notify_error"), Ok(()), "{window} can't report an error");
    }
}
