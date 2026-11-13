// Round 3E item 2: a tile shows the real icon of what it opens, never a
// letter. Apps and folders: the icon Windows itself shows for them (the
// shortcut's or exe's, a Store app's logo through its AppsFolder item, the
// folder's), drawn at 2× the size shown. Websites: the site's own icon,
// fetched once. Each is cached as a small file in Groovy's data folder: an
// app's is made again when its exe changes. The services' logo marks are in
// the page itself (views/brands.ts). Only when nothing can be found does a
// tile fall back to a letter, and that is logged once.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;

/// The largest icon kept from a website.
const MAX_SITE_ICON: usize = 256 * 1024;

fn cache_dir() -> PathBuf {
    crate::settings::local_dir().join("icons")
}

/// FNV-1a: a cache name that stays the same from one build to the next.
fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// What the icon depends on: the file and when it last changed, so a new exe
/// (an update) makes a new icon. A shortcut counts its target's change too.
fn stamp(kind: &str, target: &str) -> String {
    if kind == "site" {
        return String::new();
    }
    let mut parts = Vec::new();
    for p in [Some(target.to_string()), os::shortcut_target(target)].into_iter().flatten() {
        if let Ok(m) = std::fs::metadata(&p) {
            let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            parts.push(format!("{t}:{}", m.len()));
        }
    }
    parts.join("|")
}

pub fn cache_key(kind: &str, target: &str, px: u32) -> String {
    fnv(&format!("{kind}\n{target}\n{px}\n{}", stamp(kind, target)))
}

/// Tiles already logged as having no icon (once each per run).
static MISSED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn missed(kind: &str, target: &str) {
    let mut m = MISSED.lock().unwrap();
    if m.get_or_insert_with(HashSet::new).insert(format!("{kind}:{target}")) {
        // The name only, never a full path (it can hold the user's name).
        let name = target.rsplit(['\\', '/']).find(|s| !s.is_empty()).unwrap_or(target);
        crate::log::line(format!("tile icon: none found for {kind} \"{name}\", a letter instead"));
    }
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// The icon for a tile as a data: URL, `px` square (2× the size it is shown at).
#[tauri::command]
pub async fn tile_icon(kind: String, target: String, px: Option<u32>) -> Option<String> {
    let px = px.unwrap_or(44).clamp(16, 128);
    if !matches!(kind.as_str(), "app" | "folder" | "site") || target.trim().is_empty() {
        return None;
    }
    let key = {
        let (k, t) = (kind.clone(), target.clone());
        tauri::async_runtime::spawn_blocking(move || cache_key(&k, &t, px)).await.ok()?
    };
    let file = cache_dir().join(format!("{key}.txt"));
    if let Ok(url) = std::fs::read_to_string(&file) {
        if url.starts_with("data:image/") {
            return Some(url);
        }
    }
    let url = if kind == "site" {
        site_icon(&target).await
    } else {
        let t = target.clone();
        tauri::async_runtime::spawn_blocking(move || os::shell_icon(&t, px).map(|png| data_url("image/png", &png))).await.ok().flatten()
    };
    match url {
        Some(url) => {
            let dir = cache_dir();
            if crate::platform::ensure_private_dir(&dir).is_ok() {
                let _ = std::fs::write(&file, &url);
            }
            Some(url)
        }
        None => {
            missed(&kind, &target);
            None
        }
    }
}

// ── Websites ──────────────────────────────────────────────────────────────────

/// The icons a page names in its <head>, best first: a large touch icon,
/// then the largest declared icon, then any icon; the site's /favicon.ico last.
pub fn icon_links(html: &str, base: &reqwest::Url) -> Vec<reqwest::Url> {
    let lower = html.to_ascii_lowercase();
    let mut found: Vec<(u32, reqwest::Url)> = Vec::new();
    let mut at = 0;
    while let Some(i) = lower[at..].find("<link") {
        let start = at + i;
        let end = lower[start..].find('>').map(|e| start + e).unwrap_or(lower.len());
        let tag = &html[start..end];
        let tag_lower = &lower[start..end];
        at = end;
        let Some(rel) = attr(tag, tag_lower, "rel") else { continue };
        let rel = rel.to_ascii_lowercase();
        if !rel.split_whitespace().any(|r| r == "icon" || r == "apple-touch-icon" || r == "apple-touch-icon-precomposed") {
            continue;
        }
        let Some(href) = attr(tag, tag_lower, "href") else { continue };
        let Ok(url) = base.join(href.trim()) else { continue };
        if !matches!(url.scheme(), "https" | "http") {
            continue;
        }
        let size = attr(tag, tag_lower, "sizes")
            .and_then(|s| s.split(|c: char| c == 'x' || c == 'X').next().and_then(|n| n.trim().parse::<u32>().ok()))
            .unwrap_or(if rel.contains("apple-touch-icon") { 180 } else if href.ends_with(".svg") { 512 } else { 16 });
        found.push((size, url));
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<reqwest::Url> = found.into_iter().map(|(_, u)| u).collect();
    if let Ok(ico) = base.join("/favicon.ico") {
        if !out.contains(&ico) {
            out.push(ico);
        }
    }
    out
}

fn attr<'a>(tag: &'a str, tag_lower: &str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(i) = tag_lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        // A whole attribute name, followed by =.
        if at > 0 && !tag_lower.as_bytes()[at - 1].is_ascii_whitespace() {
            continue;
        }
        let rest = tag_lower[from..].trim_start();
        if !rest.starts_with('=') {
            continue;
        }
        let value_at = tag.len() - rest.len() + 1;
        let value = tag[value_at..].trim_start();
        let offset = tag.len() - value.len();
        return match value.chars().next() {
            Some(q @ ('"' | '\'')) => value[1..].find(q).map(|e| &tag[offset + 1..offset + 1 + e]),
            Some(_) => Some(&tag[offset..offset + value.find(|c: char| c.is_whitespace() || c == '/' || c == '>').unwrap_or(value.len())]),
            None => None,
        };
    }
    None
}

fn image_mime(bytes: &[u8], declared: &str) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else if bytes.starts_with(b"\xFF\xD8") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if declared.contains("svg") || String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).contains("<svg") {
        Some("image/svg+xml")
    } else {
        None
    }
}

async fn fetch(client: &reqwest::Client, url: &reqwest::Url, limit: usize) -> Option<(Vec<u8>, String)> {
    let mut res = client.get(url.clone()).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let mime = res.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.ok()? {
        body.extend_from_slice(&chunk);
        if body.len() > limit {
            return None;
        }
    }
    Some((body, mime))
}

async fn site_icon(target: &str) -> Option<String> {
    let mut page = reqwest::Url::parse(target.trim()).ok()?;
    if !matches!(page.scheme(), "https" | "http") || page.host_str().is_none() {
        return None;
    }
    page.set_path("/");
    page.set_query(None);
    page.set_fragment(None);
    let client = crate::net::client(&page, Duration::from_secs(8)).ok()?;
    let html = fetch(&client, &page, 512 * 1024).await.map(|(b, _)| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    for url in icon_links(&html, &page).into_iter().take(4) {
        if let Some((bytes, declared)) = fetch(&client, &url, MAX_SITE_ICON).await {
            if let Some(mime) = image_mime(&bytes, &declared) {
                return Some(data_url(mime, &bytes));
            }
        }
    }
    None
}

// ── Apps and folders: Windows' own icon ───────────────────────────────────────

/// RGBA from Windows' premultiplied BGRA (a fully transparent bitmap is opaque).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn bgra_to_rgba(mut px: Vec<u8>) -> Vec<u8> {
    let any_alpha = px.chunks_exact(4).any(|p| p[3] != 0);
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
        if !any_alpha {
            p[3] = 255;
        } else if p[3] != 0 && p[3] != 255 {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    px
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(rgba).ok()?;
    }
    Some(out)
}

#[cfg(windows)]
mod os {
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, IShellLinkW, SHCreateItemFromParsingName, ShellLink, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY};

    struct Com(bool);
    impl Com {
        fn new() -> Com {
            Com(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
        }
    }
    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    /// The exe a .lnk points to, if it is one.
    pub fn shortcut_target(path: &str) -> Option<String> {
        if !path.to_ascii_lowercase().ends_with(".lnk") {
            return None;
        }
        let _com = Com::new();
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            link.cast::<IPersistFile>().ok()?.Load(&HSTRING::from(path), STGM_READ).ok()?;
            let mut buf = [0u16; 1024];
            link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            let target = String::from_utf16_lossy(&buf[..len]);
            (!target.is_empty()).then_some(target)
        }
    }

    /// The icon Windows shows for a file, a folder, a shortcut or a Store app
    /// ("shell:AppsFolder\<AUMID>"), as a PNG `px` square.
    pub fn shell_icon(path: &str, px: u32) -> Option<Vec<u8>> {
        let _com = Com::new();
        unsafe {
            let factory: IShellItemImageFactory = SHCreateItemFromParsingName(PCWSTR(HSTRING::from(path).as_ptr()), None).ok()?;
            let bmp = factory.GetImage(SIZE { cx: px as i32, cy: px as i32 }, SIIGBF_BIGGERSIZEOK | SIIGBF_ICONONLY).ok()?;
            let mut info = BITMAP::default();
            let got = GetObjectW(HGDIOBJ(bmp.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut info as *mut _ as *mut _));
            let (w, h) = (info.bmWidth, info.bmHeight.abs());
            let mut pixels = vec![0u8; (w.max(0) * h * 4) as usize];
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let dc = CreateCompatibleDC(None);
            let lines = if got != 0 && w > 0 && h > 0 {
                GetDIBits(dc, bmp, 0, h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS)
            } else {
                0
            };
            let _ = DeleteDC(dc);
            let _ = DeleteObject(HGDIOBJ(bmp.0));
            if lines == 0 {
                return None;
            }
            super::encode_png(w as u32, h as u32, &super::bgra_to_rgba(pixels))
        }
    }
}

#[cfg(not(windows))]
mod os {
    pub fn shortcut_target(_path: &str) -> Option<String> {
        None
    }

    /// Elsewhere there is no shell icon to ask for: the tile uses its fallback.
    pub fn shell_icon(_path: &str, _px: u32) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_names_its_icons_best_first() {
        let base = reqwest::Url::parse("https://example.com/").unwrap();
        let html = r#"<html><head>
            <link rel="stylesheet" href="/a.css">
            <link rel="icon" type="image/png" sizes="32x32" href="/icon-32.png">
            <LINK REL='apple-touch-icon' HREF='/touch.png'>
            <link href="https://cdn.example.com/big.png" rel="shortcut icon" sizes="96x96"/>
            <link rel="icon" href="javascript:alert(1)">
        </head></html>"#;
        let got: Vec<String> = icon_links(html, &base).into_iter().map(|u| u.to_string()).collect();
        assert_eq!(got, ["https://example.com/touch.png", "https://cdn.example.com/big.png", "https://example.com/icon-32.png", "https://example.com/favicon.ico"]);
        assert_eq!(icon_links("", &base).len(), 1, "no page: the site's /favicon.ico");
    }

    #[test]
    fn only_images_are_kept() {
        assert_eq!(image_mime(b"\x89PNG\r\n", ""), Some("image/png"));
        assert_eq!(image_mime(&[0, 0, 1, 0, 1], ""), Some("image/x-icon"));
        assert_eq!(image_mime(b"<svg xmlns=", "text/plain"), Some("image/svg+xml"));
        assert_eq!(image_mime(b"<!doctype html>", "text/html"), None, "a 404 page isn't an icon");
    }

    #[test]
    fn shell_pixels_become_a_png() {
        // One half-transparent premultiplied red pixel, one opaque blue.
        let rgba = bgra_to_rgba(vec![0, 0, 128, 128, 255, 0, 0, 255]);
        assert_eq!(rgba, vec![255, 0, 0, 128, 0, 0, 255, 255]);
        assert_eq!(bgra_to_rgba(vec![1, 2, 3, 0])[3], 255, "no alpha at all: opaque");
        let png = encode_png(2, 1, &rgba).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn the_cache_name_follows_the_file() {
        let dir = std::env::temp_dir().join(format!("groovy-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("app.exe");
        std::fs::write(&exe, b"one").unwrap();
        let path = exe.to_string_lossy().to_string();
        let a = cache_key("app", &path, 44);
        assert_eq!(a, cache_key("app", &path, 44), "the same file, the same name");
        assert_ne!(a, cache_key("app", &path, 64), "another size");
        std::fs::write(&exe, b"an update").unwrap();
        assert_ne!(a, cache_key("app", &path, 44), "a new exe makes a new icon");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(fnv("groovy"), fnv("groovy"));
    }
}
