//! t975 (plugin-api-stage1, D6; capability `plugins`): the crate graph keeps plugins at the leaves.
//!
//! The owner's reason for plugins first is build time; a graph rule no test checks is broken by
//! the first convenient `use`. The manifests are read as text (no cargo invocation).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// The workspace members: (package name, path relative to src/).
fn members() -> Vec<(String, String)> {
    let ws = std::fs::read_to_string(src().join("Cargo.toml")).unwrap();
    let block = &ws[ws.find("members = [").unwrap()..];
    let block = &block[..block.find(']').unwrap()];
    block
        .lines()
        .filter_map(|l| l.trim().strip_prefix('"').and_then(|l| l.split('"').next()))
        .map(|path| {
            let man = std::fs::read_to_string(src().join(path).join("Cargo.toml")).unwrap();
            let name = man
                .lines()
                .find_map(|l| l.strip_prefix("name = \"").map(|n| n.trim_end_matches('"').to_string()))
                .unwrap();
            (name, path.to_string())
        })
        .collect()
}

/// The dependencies of a manifest by kind: "normal" (incl. target-specific), "build", "dev".
fn deps(path: &str) -> BTreeMap<&'static str, Vec<String>> {
    let man = std::fs::read_to_string(src().join(path).join("Cargo.toml")).unwrap();
    let mut out: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut kind: Option<&'static str> = None;
    for l in man.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            kind = if t.ends_with("dev-dependencies]") {
                Some("dev")
            } else if t.ends_with("build-dependencies]") {
                Some("build")
            } else if t.ends_with("dependencies]") {
                Some("normal")
            } else {
                None
            };
            continue;
        }
        let Some(k) = kind else { continue };
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let name = t.split(|c: char| c == '=' || c == '.' || c.is_whitespace()).next().unwrap();
        out.entry(k).or_default().push(name.to_string());
    }
    out
}

const CORE: &[&str] = &["kt-model", "kt-fx", "kt-engine", "kt-gpu", "kt-media", "kt-cache", "kt-otio"];
/// What a family crate may depend on besides the API: the text crate for the title (D6).
const FAMILY_MAY_USE: &[&str] = &["kt-plugin-api", "kt-text"];

#[test]
fn t975_the_crate_graph_keeps_plugins_at_the_leaves() {
    let members = members();
    let families: Vec<&(String, String)> = members.iter().filter(|(_, p)| p.starts_with("plugins/")).collect();
    assert!(!families.is_empty(), "no family crate under src/plugins/");
    let is_plugin_side = |n: &str| n == "kt-plugins-builtin" || families.iter().any(|(f, _)| f == n);

    let api = deps("libs/kt-plugin-api");
    let mut api_normal = api.get("normal").cloned().unwrap_or_default();
    api_normal.sort();
    assert_eq!(api_normal, vec!["kt-time", "serde"], "the plugin API must depend on kt-time and serde only");

    for (name, path) in &members {
        let d = deps(path);
        let normal_build: Vec<&String> = d.get("normal").into_iter().chain(d.get("build")).flatten().collect();
        if CORE.contains(&name.as_str()) {
            for dep in &normal_build {
                assert!(!is_plugin_side(dep), "core crate {name} depends on {dep}: plugins must stay leaves");
            }
        }
        if path.starts_with("plugins/") {
            assert!(normal_build.iter().any(|d| *d == "kt-plugin-api"), "family {name} does not depend on the API");
            for dep in &normal_build {
                assert!(FAMILY_MAY_USE.contains(&dep.as_str()), "family {name} depends on {dep}, outside the closed list");
            }
        }
        if !matches!(name.as_str(), "kt-app" | "kt-cli") {
            assert!(
                !d.get("normal").into_iter().flatten().any(|x| x == "kt-plugins-builtin"),
                "{name} depends on the assembler: only kt-app and kt-cli may (test targets as a dev-dependency)"
            );
        }
    }
    for app in ["apps/kt-app", "apps/kt-cli"] {
        assert!(
            deps(app).get("normal").into_iter().flatten().any(|x| x == "kt-plugins-builtin"),
            "{app} does not install the built-in set"
        );
    }
}
