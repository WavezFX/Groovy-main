fn main() {
    // The app's manifest (Common Controls v6, as tauri-build's default) is
    // embedded by the linker instead of tauri-build's resource, so the unit
    // test binary gets it too: without it, a test executable that links a
    // v6-only function (TaskDialogIndirect, LoadIconMetric) can't even start
    // (STATUS_ENTRYPOINT_NOT_FOUND).
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    // Round 3G item 2: every command Groovy registers gets an allow/deny
    // permission, so a window may call only what its capability allows
    // (capabilities/<window>.json, from scripts/command-access.mjs).
    let manifest = tauri_build::AppManifest::new().commands(registered_commands());
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows).app_manifest(manifest)).expect("tauri-build failed");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows-msvc") {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
}

/// The command names in lib.rs's generate_handler! list, read at build time.
fn registered_commands() -> &'static [&'static str] {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=capabilities");
    let lib = std::fs::read_to_string("src/lib.rs").expect("src/lib.rs");
    let start = lib.find("generate_handler![").expect("generate_handler! in lib.rs") + "generate_handler![".len();
    let block = &lib[start..start + lib[start..].find(']').expect("the end of generate_handler!")];
    let names: Vec<&'static str> = block
        .split(',')
        .filter_map(|item| item.trim().rsplit("::").next())
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
        .map(|name| &*Box::leak(name.to_string().into_boxed_str()))
        .collect();
    assert!(names.len() > 100, "generate_handler! read as {} commands", names.len());
    Box::leak(names.into_boxed_slice())
}
