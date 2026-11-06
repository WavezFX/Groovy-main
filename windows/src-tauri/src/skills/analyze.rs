// A project's profile, for recommending skills: its manifests and what they
// depend on, the languages by file count, the test frameworks, linters and
// formatters, CLAUDE.md / AGENTS.md, and what changed in git over the last 30
// days. The manifests' hash says when it is worth analysing again.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfile {
    pub name: String,
    pub path: String,
    /// Manifest files found, relative to the project.
    pub manifests: Vec<String>,
    /// Dependency names from every manifest, lower case.
    pub dependencies: Vec<String>,
    /// npm script names.
    pub scripts: Vec<String>,
    /// Extension → number of files, the biggest first (at most 12).
    pub languages: Vec<(String, usize)>,
    /// Frameworks and platforms recognised (react, tauri, django…).
    pub frameworks: Vec<String>,
    pub test_frameworks: Vec<String>,
    pub linters: Vec<String>,
    pub has_claude_md: bool,
    pub has_agents_md: bool,
    pub ci: Vec<String>,
    /// Top-level folders with commits in the last 30 days, the busiest first.
    pub active_areas: Vec<String>,
    /// SHA-256 of the manifests' contents.
    pub hash: String,
}

const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "dist", "build", "out", ".next", ".nuxt", "vendor", "__pycache__", ".venv", "venv",
    ".idea", ".vscode", ".groovy", ".agents", ".claude", "coverage", ".turbo", ".cache", "bin", "obj",
];
const MAX_FILES: usize = 30_000;
const MAX_DEPTH: usize = 8;

const MANIFESTS: &[&str] = &[
    "package.json", "Cargo.toml", "pyproject.toml", "requirements.txt", "go.mod", "Gemfile", "composer.json",
    "pom.xml", "build.gradle", "build.gradle.kts", "tauri.conf.json", "Dockerfile", "docker-compose.yml",
    "deno.json", "pubspec.yaml", "Package.swift",
];

/// What a dependency or file says about the stack: (needle, framework).
const FRAMEWORKS: &[(&str, &str)] = &[
    ("react", "react"), ("next", "nextjs"), ("vue", "vue"), ("nuxt", "nuxt"), ("svelte", "svelte"), ("@angular/core", "angular"),
    ("solid-js", "solid"), ("astro", "astro"), ("express", "express"), ("fastify", "fastify"), ("@nestjs/core", "nestjs"),
    ("tailwindcss", "tailwind"), ("@tauri-apps/api", "tauri"), ("tauri", "tauri"), ("electron", "electron"),
    ("react-native", "react-native"), ("expo", "expo"), ("prisma", "prisma"), ("drizzle-orm", "drizzle"),
    ("@supabase/supabase-js", "supabase"), ("stripe", "stripe"), ("django", "django"), ("flask", "flask"),
    ("fastapi", "fastapi"), ("torch", "pytorch"), ("tensorflow", "tensorflow"), ("pandas", "pandas"),
    ("langchain", "langchain"), ("ollama", "ollama"), ("@anthropic-ai/sdk", "anthropic"), ("anthropic", "anthropic"),
    ("openai", "openai"), ("tokio", "tokio"), ("axum", "axum"), ("actix-web", "actix"), ("bevy", "bevy"),
    ("three", "threejs"), ("vite", "vite"), ("graphql", "graphql"), ("redis", "redis"), ("mongodb", "mongodb"),
    ("pg", "postgres"), ("sqlx", "sqlx"), ("rusqlite", "sqlite"), ("gin-gonic", "gin"), ("rails", "rails"),
    ("laravel/framework", "laravel"), ("flutter", "flutter"), ("mcp", "mcp"), ("@modelcontextprotocol/sdk", "mcp"),
];
const TESTS: &[&str] = &[
    "jest", "vitest", "mocha", "playwright", "@playwright/test", "cypress", "pytest", "unittest", "rspec", "phpunit",
    "@testing-library/react", "ava", "tap", "proptest", "insta", "criterion",
];
const LINTERS: &[&str] = &[
    "eslint", "prettier", "biome", "@biomejs/biome", "ruff", "black", "flake8", "mypy", "pylint", "clippy", "rustfmt",
    "stylelint", "golangci-lint", "rubocop", "oxlint",
];

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Every file under `root`, skipping build output and dependency folders.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                if depth < MAX_DEPTH && !SKIP_DIRS.contains(&file_name(&p).as_str()) {
                    stack.push((p, depth + 1));
                }
            } else if ft.is_file() {
                out.push(p);
                if out.len() >= MAX_FILES {
                    return out;
                }
            }
        }
    }
    out
}

fn deps_from_package_json(text: &str, deps: &mut BTreeSet<String>, scripts: &mut BTreeSet<String>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
    for key in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        if let Some(o) = v[key].as_object() {
            deps.extend(o.keys().map(|k| k.to_lowercase()));
        }
    }
    if let Some(o) = v["scripts"].as_object() {
        scripts.extend(o.keys().cloned());
    }
}

fn deps_from_cargo(text: &str, deps: &mut BTreeSet<String>) {
    let mut in_deps = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_deps = l.contains("dependencies");
            // [dependencies.serde]
            if let Some(name) = l.trim_matches(['[', ']']).strip_prefix("dependencies.") {
                deps.insert(name.trim().to_lowercase());
            }
            continue;
        }
        if in_deps {
            if let Some((name, _)) = l.split_once('=') {
                let name = name.trim();
                if !name.is_empty() && !name.starts_with('#') {
                    deps.insert(name.to_lowercase());
                }
            }
        }
    }
}

/// requirements.txt lines and pyproject's dependency strings: "django>=4" → django.
fn python_name(spec: &str) -> Option<String> {
    let s = spec.trim().trim_matches(['"', '\'', ',']).trim();
    if s.is_empty() || s.starts_with('#') || s.starts_with('-') {
        return None;
    }
    let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))).unwrap_or(s.len());
    let name = &s[..end];
    (!name.is_empty()).then(|| name.to_lowercase())
}

fn deps_from_python(text: &str, toml: bool, deps: &mut BTreeSet<String>) {
    if !toml {
        deps.extend(text.lines().filter_map(python_name));
        return;
    }
    // pyproject: the quoted strings inside dependency arrays, and poetry tables.
    let mut in_array = false;
    let mut in_table = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') && !in_array {
            in_table = l.contains("dependencies");
            continue;
        }
        if l.starts_with("dependencies") && l.contains('[') || l.contains("-dependencies") && l.contains("= [") {
            in_array = !l.contains(']');
            for part in l.split('[').nth(1).unwrap_or("").split(',') {
                deps.extend(python_name(part.trim_end_matches(']')));
            }
            continue;
        }
        if in_array {
            if l.starts_with(']') {
                in_array = false;
                continue;
            }
            deps.extend(python_name(l));
        } else if in_table {
            if let Some((name, _)) = l.split_once('=') {
                if name.trim() != "python" {
                    deps.extend(python_name(name));
                }
            }
        }
    }
}

fn deps_from_go(text: &str, deps: &mut BTreeSet<String>) {
    for line in text.lines() {
        let l = line.trim().trim_start_matches("require").trim();
        if let Some(module) = l.split_whitespace().next() {
            if module.contains('.') && module.contains('/') {
                deps.insert(module.rsplit('/').next().unwrap_or(module).to_lowercase());
                if let Some(owner) = module.split('/').nth(1) {
                    deps.insert(owner.to_lowercase());
                }
            }
        }
    }
}

/// Top-level folders touched by commits in the last 30 days, busiest first.
fn active_areas(root: &Path) -> Vec<String> {
    if !root.join(".git").exists() {
        return Vec::new();
    }
    let mut cmd = std::process::Command::new("git");
    cmd.args(["log", "--since=30.days", "--name-only", "--pretty=format:", "-n", "300"]).current_dir(root);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let Ok(out) = cmd.output() else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    areas_from_log(&String::from_utf8_lossy(&out.stdout))
}

pub fn areas_from_log(log: &str) -> Vec<String> {
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    for line in log.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let area = match line.split_once('/') {
            Some((top, _)) => top.to_string(),
            None => "(root)".to_string(),
        };
        *count.entry(area).or_default() += 1;
    }
    let mut v: Vec<(String, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().take(8).map(|(a, _)| a).collect()
}

/// Analyses `root`. Reads files only; nothing is written.
pub fn analyze(root: &Path) -> ProjectProfile {
    let files = walk(root);
    let mut deps = BTreeSet::new();
    let mut scripts = BTreeSet::new();
    let mut manifests = Vec::new();
    let mut ext_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut frameworks = BTreeSet::new();
    let mut ci = BTreeSet::new();
    let mut hasher = ring::digest::Context::new(&ring::digest::SHA256);
    let mut manifest_files: Vec<&PathBuf> = Vec::new();

    for f in &files {
        let name = file_name(f);
        let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        if let Some(ext) = f.extension().map(|e| e.to_string_lossy().to_lowercase()) {
            *ext_count.entry(ext).or_default() += 1;
        }
        let is_manifest = MANIFESTS.contains(&name.as_str()) || name.ends_with(".csproj") || name.ends_with(".fsproj");
        if is_manifest {
            manifest_files.push(f);
        }
        if rel.starts_with(".github/workflows/") {
            ci.insert("github-actions".to_string());
            manifest_files.push(f);
        } else if rel == ".gitlab-ci.yml" {
            ci.insert("gitlab-ci".to_string());
        }
        match name.as_str() {
            "Dockerfile" | "docker-compose.yml" => {
                frameworks.insert("docker".to_string());
            }
            "tauri.conf.json" => {
                frameworks.insert("tauri".to_string());
            }
            _ if name.ends_with(".csproj") => {
                frameworks.insert("dotnet".to_string());
            }
            _ => {}
        }
    }
    manifest_files.sort();
    for f in &manifest_files {
        let name = file_name(f);
        let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        if text.len() > 2_000_000 {
            continue;
        }
        hasher.update(rel.as_bytes());
        hasher.update(text.as_bytes());
        if !rel.starts_with(".github/") {
            manifests.push(rel);
        }
        match name.as_str() {
            "package.json" => deps_from_package_json(&text, &mut deps, &mut scripts),
            "Cargo.toml" => deps_from_cargo(&text, &mut deps),
            "requirements.txt" => deps_from_python(&text, false, &mut deps),
            "pyproject.toml" => deps_from_python(&text, true, &mut deps),
            "go.mod" => deps_from_go(&text, &mut deps),
            _ => {}
        }
    }
    for (needle, fw) in FRAMEWORKS {
        if deps.contains(*needle) {
            frameworks.insert((*fw).to_string());
        }
    }
    let test_frameworks: Vec<String> = TESTS.iter().filter(|t| deps.contains(**t)).map(|t| t.trim_start_matches('@').split('/').next().unwrap_or(t).to_string()).collect::<BTreeSet<_>>().into_iter().collect();
    let mut linters: BTreeSet<String> = LINTERS.iter().filter(|l| deps.contains(**l)).map(|l| l.trim_start_matches('@').split('/').next().unwrap_or(l).to_string()).collect();
    if manifests.iter().any(|m| m.ends_with("Cargo.toml")) {
        linters.insert("clippy".into());
    }
    for f in &files {
        let n = file_name(f);
        if n.starts_with(".eslintrc") || n.starts_with("eslint.config") {
            linters.insert("eslint".into());
        } else if n.starts_with(".prettierrc") || n.starts_with("prettier.config") {
            linters.insert("prettier".into());
        } else if n == "biome.json" {
            linters.insert("biome".into());
        } else if n == "ruff.toml" || n == ".ruff.toml" {
            linters.insert("ruff".into());
        }
    }
    let mut languages: Vec<(String, usize)> = ext_count
        .into_iter()
        .filter(|(e, _)| !matches!(e.as_str(), "json" | "lock" | "md" | "txt" | "png" | "jpg" | "svg" | "ico" | "gif" | "map" | "yml" | "yaml" | "toml"))
        .collect();
    languages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    languages.truncate(12);

    ProjectProfile {
        name: file_name(root),
        path: root.to_string_lossy().into_owned(),
        manifests,
        dependencies: deps.into_iter().collect(),
        scripts: scripts.into_iter().collect(),
        languages,
        frameworks: frameworks.into_iter().collect(),
        test_frameworks,
        linters: linters.into_iter().collect(),
        has_claude_md: root.join("CLAUDE.md").is_file(),
        has_agents_md: root.join("AGENTS.md").is_file(),
        ci: ci.into_iter().collect(),
        active_areas: active_areas(root),
        hash: super::hex(hasher.finish().as_ref()),
    }
}

/// Only the manifests' hash, to see whether analysing again is worth it.
/// (The same walk as analyze(): cheap enough, and one definition of "manifest".)
pub fn manifest_hash(root: &Path) -> String {
    analyze(root).hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(files: &[(&str, &str)]) -> super::super::TestDir {
        let dir = super::super::TestDir::new("analyze");
        for (path, content) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    #[test]
    fn a_tauri_react_project() {
        let dir = fixture(&[
            ("package.json", r#"{"dependencies":{"react":"18","@tauri-apps/api":"2"},"devDependencies":{"vitest":"1","eslint":"9"},"scripts":{"test":"vitest","build":"vite build"}}"#),
            ("src-tauri/Cargo.toml", "[package]\nname = \"x\"\n\n[dependencies]\ntauri = \"2\"\ntokio = { version = \"1\" }\n\n[dependencies.serde]\nversion = \"1\"\n"),
            ("src-tauri/tauri.conf.json", "{}"),
            ("src/App.tsx", ""),
            ("src/main.tsx", ""),
            ("src-tauri/src/main.rs", ""),
            ("node_modules/react/index.js", ""),
            ("CLAUDE.md", "# rules"),
            (".github/workflows/ci.yml", "on: push"),
        ]);
        let p = analyze(dir.path());
        assert!(p.frameworks.contains(&"react".to_string()));
        assert!(p.frameworks.contains(&"tauri".to_string()));
        assert!(p.frameworks.contains(&"tokio".to_string()));
        assert!(p.dependencies.contains(&"serde".to_string()));
        assert_eq!(p.test_frameworks, vec!["vitest"]);
        assert!(p.linters.contains(&"eslint".to_string()) && p.linters.contains(&"clippy".to_string()));
        assert_eq!(p.languages[0], ("tsx".to_string(), 2));
        assert!(p.has_claude_md && !p.has_agents_md);
        assert_eq!(p.ci, vec!["github-actions"]);
        assert_eq!(p.scripts, vec!["build", "test"]);
        // node_modules is not walked.
        assert!(!p.languages.iter().any(|(e, _)| e == "js"));
        assert!(p.manifests.contains(&"package.json".to_string()));
    }

    #[test]
    fn a_python_project() {
        let dir = fixture(&[
            ("requirements.txt", "Django>=4.2\n# comment\npytest==8\n-r other.txt\n"),
            ("pyproject.toml", "[project]\nname = \"x\"\ndependencies = [\n  \"fastapi>=0.1\",\n  \"ruff\",\n]\n"),
            ("app/main.py", ""),
        ]);
        let p = analyze(dir.path());
        for d in ["django", "pytest", "fastapi", "ruff"] {
            assert!(p.dependencies.contains(&d.to_string()), "{d} in {:?}", p.dependencies);
        }
        assert!(p.frameworks.contains(&"django".to_string()) && p.frameworks.contains(&"fastapi".to_string()));
        assert_eq!(p.test_frameworks, vec!["pytest"]);
    }

    #[test]
    fn the_hash_follows_the_manifests_only() {
        let dir = fixture(&[("package.json", r#"{"dependencies":{"react":"18"}}"#), ("src/a.ts", "1")]);
        let a = manifest_hash(dir.path());
        std::fs::write(dir.path().join("src/a.ts"), "2").unwrap();
        assert_eq!(a, manifest_hash(dir.path()));
        std::fs::write(dir.path().join("package.json"), r#"{"dependencies":{"vue":"3"}}"#).unwrap();
        assert_ne!(a, manifest_hash(dir.path()));
    }

    #[test]
    fn git_areas() {
        let log = "src/a.rs\nsrc/b.rs\n\ndocs/x.md\nREADME.md\nsrc/c.rs\n";
        assert_eq!(areas_from_log(log), vec!["src", "(root)", "docs"]);
    }
}
