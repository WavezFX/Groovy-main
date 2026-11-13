// Recommending skills for a project: search queries from its profile, the
// skills.sh search for each, then a ranking by Groovy's AI (the local model by
// default, brain_ai.rs) against the profile. With no AI, or an answer that
// can't be used, rules match dependency names with skill names. Only
// candidates that really came out of the search can be recommended: the model
// can rank them, never invent one.

use std::collections::BTreeSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::analyze::ProjectProfile;
use super::cli::Installed;

pub const SEARCH_API: &str = "https://skills.sh/api/search";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub installs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRec {
    pub source: String,
    pub skill: String,
    pub reason: String,
    /// 0…1
    pub confidence: f32,
    #[serde(default)]
    pub installs: u64,
}

impl InstallRec {
    pub fn id(&self) -> String {
        format!("install:{}@{}", self.source, self.skill)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRec {
    pub skill: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Recommendations {
    pub install: Vec<InstallRec>,
    pub remove: Vec<RemoveRec>,
    pub keep: Vec<String>,
    /// "ai" or "rules"
    pub by: String,
    pub at: u64,
    /// The searches that could not be made (offline…), shown to the user.
    pub problems: Vec<String>,
}

fn language_name(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" => "javascript",
        "py" => "python",
        "go" => "go",
        "rb" => "ruby",
        "php" => "php",
        "java" => "java",
        "kt" => "kotlin",
        "swift" => "swift",
        "cs" => "csharp",
        "cpp" | "cc" | "hpp" => "cpp",
        "c" | "h" => "c",
        "dart" => "dart",
        "vue" => "vue",
        "svelte" => "svelte",
        _ => return None,
    })
}

/// The main languages of the project, by name.
pub fn languages(profile: &ProjectProfile) -> Vec<String> {
    let mut seen = BTreeSet::new();
    profile.languages.iter().filter_map(|(e, _)| language_name(e)).filter(|l| seen.insert(*l)).take(3).map(String::from).collect()
}

/// What to search for, the most telling first (at most 6).
pub fn queries(profile: &ProjectProfile) -> Vec<String> {
    let mut q: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !s.is_empty() && !q.contains(&s) && q.len() < 6 {
            q.push(s);
        }
    };
    let langs = languages(profile);
    for fw in &profile.frameworks {
        if !matches!(fw.as_str(), "vite" | "docker" | "tokio") {
            push(fw.clone());
        }
    }
    if let Some(t) = profile.test_frameworks.first() {
        push(t.clone());
    } else if let Some(l) = langs.first() {
        push(format!("{l} testing"));
    }
    for l in &langs {
        push(l.clone());
    }
    if profile.frameworks.iter().any(|f| f == "docker") {
        push("docker".into());
    }
    if !profile.ci.is_empty() {
        push("github actions".into());
    }
    q
}

#[derive(Deserialize)]
struct SearchAnswer {
    #[serde(default)]
    skills: Vec<Candidate>,
}

pub fn parse_search(body: &str) -> Vec<Candidate> {
    let mut v = serde_json::from_str::<SearchAnswer>(body).map(|a| a.skills).unwrap_or_default();
    v.retain(|c| super::cli::valid_source(&c.source) && super::cli::valid_skill(&c.name));
    v.sort_by(|a, b| b.installs.cmp(&a.installs));
    v
}

/// skills.sh search. Nothing personal is sent: only the query.
pub async fn search(query: &str, limit: usize) -> Result<Vec<Candidate>, String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(12)).build().map_err(|e| e.to_string())?;
    let resp = client
        .get(SEARCH_API)
        .query(&[("q", query), ("limit", &limit.to_string())])
        .header("User-Agent", "Groovy")
        .send()
        .await
        .map_err(|e| format!("skills.sh could not be reached: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("skills.sh answered {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    Ok(parse_search(&body))
}

fn words(s: &str) -> BTreeSet<String> {
    s.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 2).map(String::from).collect()
}

/// The stack's words: frameworks, languages, test tools, linters and dependencies.
fn stack(profile: &ProjectProfile) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = BTreeSet::new();
    for x in profile.frameworks.iter().chain(&profile.test_frameworks).chain(&profile.linters).chain(&languages(profile)) {
        s.extend(words(x));
        s.insert(x.to_lowercase());
    }
    for d in &profile.dependencies {
        s.insert(d.trim_start_matches('@').split('/').next_back().unwrap_or(d).to_string());
    }
    s
}

/// Technologies a skill can be about; one the project doesn't use makes it a removal candidate.
const TECH: &[&str] = &[
    "react", "nextjs", "vue", "nuxt", "svelte", "angular", "solid", "astro", "tauri", "electron", "django", "flask",
    "fastapi", "rails", "laravel", "flutter", "swift", "swiftui", "kotlin", "android", "ios", "rust", "python", "golang",
    "typescript", "php", "ruby", "java", "dotnet", "csharp", "prisma", "supabase", "stripe", "playwright", "cypress",
    "jest", "vitest", "pytest", "tailwind", "expo", "pytorch", "tensorflow", "docker", "kubernetes", "terraform",
];

fn tech_of(name_and_description: &str) -> Vec<&'static str> {
    let w = words(&name_and_description.replace("next.js", "nextjs").replace(".net", "dotnet"));
    TECH.iter().copied().filter(|t| w.contains(*t)).collect()
}

/// Rule-based recommendations: a candidate whose name matches the stack, the
/// most installed first; an installed skill about a technology the project
/// doesn't use may go.
pub fn rule_rank(profile: &ProjectProfile, candidates: &[Candidate], installed: &[(Installed, String)]) -> Recommendations {
    let stack = stack(profile);
    let have: BTreeSet<&str> = installed.iter().map(|(i, _)| i.name.as_str()).collect();
    let mut install: Vec<InstallRec> = Vec::new();
    for c in candidates {
        if have.contains(c.name.as_str()) || install.iter().any(|r| r.skill == c.name) {
            continue;
        }
        let hits: Vec<String> = words(&c.name).into_iter().filter(|w| stack.contains(w)).collect();
        if hits.is_empty() {
            continue;
        }
        let popularity = ((c.installs as f32 + 1.0).log10() / 5.0).min(1.0);
        let confidence = (0.35 + 0.15 * hits.len() as f32 + 0.3 * popularity).min(0.9);
        install.push(InstallRec {
            source: c.source.clone(),
            skill: c.name.clone(),
            reason: format!("Matches {} in this project.", hits.join(", ")),
            confidence,
            installs: c.installs,
        });
    }
    install.sort_by(|a, b| b.confidence.total_cmp(&a.confidence).then(b.installs.cmp(&a.installs)));
    install.truncate(8);
    let mut remove = Vec::new();
    let mut keep = Vec::new();
    for (skill, description) in installed {
        let techs = tech_of(&format!("{} {}", skill.name, description));
        let unused: Vec<&str> = techs.iter().copied().filter(|t| !stack.contains(*t)).collect();
        if !techs.is_empty() && unused.len() == techs.len() {
            remove.push(RemoveRec { skill: skill.name.clone(), reason: format!("This project doesn't use {}.", unused.join(", ")) });
        } else {
            keep.push(skill.name.clone());
        }
    }
    Recommendations { install, remove, keep, by: "rules".into(), at: super::store::now_ms(), problems: Vec::new() }
}

pub const RANK_SYSTEM: &str = "You recommend agent skills (SKILL.md instruction packages for coding agents) for one software project. \
You get the project's profile, candidate skills from a search, and the skills already installed. \
Answer with JSON only, no prose: {\"install\":[{\"source\":\"owner/repo\",\"skill\":\"name\",\"reason\":\"one short sentence\",\"confidence\":0.0}],\
\"remove\":[{\"skill\":\"name\",\"reason\":\"one short sentence\"}],\"keep\":[\"name\"]}. \
Only pick candidates from the list, at most 6, only ones clearly useful for this stack, confidence between 0 and 1. \
Suggest removing an installed skill only when it is clearly about a technology this project does not use.";

pub fn rank_prompt(profile: &ProjectProfile, candidates: &[Candidate], installed: &[(Installed, String)]) -> String {
    let profile = serde_json::json!({
        "name": profile.name,
        "frameworks": profile.frameworks,
        "languages": languages(profile),
        "tests": profile.test_frameworks,
        "linters": profile.linters,
        "dependencies": profile.dependencies.iter().take(60).collect::<Vec<_>>(),
        "activeAreas": profile.active_areas,
    });
    let cands: Vec<_> = candidates.iter().take(40).map(|c| serde_json::json!({ "source": c.source, "skill": c.name, "installs": c.installs })).collect();
    let inst: Vec<_> = installed.iter().map(|(i, d)| serde_json::json!({ "skill": i.name, "description": d.chars().take(200).collect::<String>() })).collect();
    format!("Project: {profile}\nCandidates: {}\nInstalled: {}", serde_json::Value::Array(cands), serde_json::Value::Array(inst))
}

/// The model's answer, kept only where it names real candidates and installed skills.
pub fn parse_ranking(answer: &str, candidates: &[Candidate], installed: &[(Installed, String)]) -> Option<Recommendations> {
    let start = answer.find('{')?;
    let end = answer.rfind('}')?;
    let v: serde_json::Value = serde_json::from_str(&answer[start..=end]).ok()?;
    let mut install = Vec::new();
    for r in v["install"].as_array()? {
        let (Some(source), Some(skill)) = (r["source"].as_str(), r["skill"].as_str()) else { continue };
        let Some(c) = candidates.iter().find(|c| c.source == source && c.name == skill) else { continue };
        if installed.iter().any(|(i, _)| i.name == c.name) || install.iter().any(|x: &InstallRec| x.skill == c.name) {
            continue;
        }
        let confidence = r["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0) as f32;
        let reason: String = r["reason"].as_str().unwrap_or("").chars().take(200).collect();
        install.push(InstallRec { source: c.source.clone(), skill: c.name.clone(), reason, confidence, installs: c.installs });
    }
    install.truncate(6);
    let names: BTreeSet<&str> = installed.iter().map(|(i, _)| i.name.as_str()).collect();
    let remove: Vec<RemoveRec> = v["remove"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let skill = r["skill"].as_str()?;
                    names.contains(skill).then(|| RemoveRec { skill: skill.into(), reason: r["reason"].as_str().unwrap_or("").chars().take(200).collect() })
                })
                .collect()
        })
        .unwrap_or_default();
    let keep = names.iter().filter(|n| !remove.iter().any(|r| r.skill == **n)).map(|n| n.to_string()).collect();
    Some(Recommendations { install, remove, keep, by: "ai".into(), at: super::store::now_ms(), problems: Vec::new() })
}

/// Searches, then ranks with Groovy's AI or the rules.
pub async fn recommend(profile: &ProjectProfile, installed: &[(Installed, String)]) -> Recommendations {
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut problems = Vec::new();
    for q in queries(profile) {
        match search(&q, 10).await {
            Ok(found) => {
                for c in found {
                    if !candidates.iter().any(|x| x.source == c.source && x.name == c.name) {
                        candidates.push(c);
                    }
                }
            }
            Err(e) => {
                problems.push(e);
                break;
            }
        }
    }
    let mut recs = None;
    if !candidates.is_empty() && crate::brain_ai::available() {
        match crate::brain_ai::complete(RANK_SYSTEM, &rank_prompt(profile, &candidates, installed), 1500, Duration::from_secs(120)).await {
            Ok(answer) => recs = parse_ranking(&answer, &candidates, installed),
            Err(e) => crate::log::line(format!("skills: ranking failed: {e}")),
        }
    }
    let mut recs = recs.unwrap_or_else(|| rule_rank(profile, &candidates, installed));
    recs.problems = problems;
    recs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::cli::Scope;

    fn profile() -> ProjectProfile {
        ProjectProfile {
            name: "app".into(),
            frameworks: vec!["react".into(), "tauri".into(), "vite".into()],
            test_frameworks: vec!["vitest".into()],
            languages: vec![("tsx".into(), 40), ("rs".into(), 20)],
            dependencies: vec!["react".into(), "@tauri-apps/api".into(), "vitest".into()],
            ..Default::default()
        }
    }

    fn cand(source: &str, name: &str, installs: u64) -> Candidate {
        Candidate { id: format!("{source}/{name}"), name: name.into(), source: source.into(), installs }
    }

    fn inst(name: &str) -> Installed {
        Installed { name: name.into(), path: format!("/p/{name}"), scope: Scope::Project, agents: vec![], source: None, source_url: None, source_type: None }
    }

    #[test]
    fn queries_follow_the_stack() {
        assert_eq!(queries(&profile()), vec!["react", "tauri", "vitest", "typescript", "rust"]);
        let empty = ProjectProfile { languages: vec![("py".into(), 3)], ..Default::default() };
        assert_eq!(queries(&empty), vec!["python testing", "python"]);
    }

    #[test]
    fn search_answers_are_checked() {
        let body = r#"{"skills":[{"id":"a/b/x","name":"x","installs":5,"source":"a/b"},{"id":"bad","name":"--all","installs":9,"source":"a/b"},{"id":"c","name":"y","installs":50,"source":"c/d"}]}"#;
        let v = parse_search(body);
        assert_eq!(v.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["y", "x"]);
        assert!(parse_search("not json").is_empty());
    }

    #[test]
    fn rules_match_names_with_the_stack() {
        let cands = vec![cand("o/r", "react-best-practices", 5000), cand("o/r", "django-helper", 9000), cand("o/r", "vitest-testing", 10)];
        let installed = vec![(inst("swiftui-tips"), "SwiftUI views for iOS".to_string()), (inst("my-notes"), "house rules".to_string())];
        let r = rule_rank(&profile(), &cands, &installed);
        assert_eq!(r.by, "rules");
        let names: Vec<_> = r.install.iter().map(|i| i.skill.as_str()).collect();
        assert_eq!(names, vec!["react-best-practices", "vitest-testing"]);
        assert_eq!(r.remove.len(), 1);
        assert_eq!(r.remove[0].skill, "swiftui-tips");
        assert_eq!(r.keep, vec!["my-notes"]);
    }

    #[test]
    fn the_model_can_rank_but_not_invent() {
        let cands = vec![cand("o/r", "react-best-practices", 5000)];
        let installed = vec![(inst("swiftui-tips"), String::new())];
        let answer = r#"Sure! {"install":[{"source":"o/r","skill":"react-best-practices","reason":"React app","confidence":0.9},
            {"source":"evil/repo","skill":"backdoor","reason":"x","confidence":1}],
            "remove":[{"skill":"swiftui-tips","reason":"no Swift"},{"skill":"not-installed","reason":"x"}],"keep":[]}"#;
        let r = parse_ranking(answer, &cands, &installed).unwrap();
        assert_eq!(r.install.len(), 1);
        assert_eq!(r.install[0].skill, "react-best-practices");
        assert_eq!(r.remove.len(), 1);
        assert_eq!(r.by, "ai");
        assert!(parse_ranking("I can't", &cands, &installed).is_none());
        assert_eq!(r.install[0].id(), "install:o/r@react-best-practices");
    }
}
