//! What projects draw on, read from their manifests.
//!
//! Links in the record say why two projects are tied, and somebody has to write
//! them. What one project actually builds against is already written down in
//! its `Cargo.toml` and `package.json`, kept true by the build, and that is the
//! list that decides who a release of a library reaches. A link kept by hand
//! named two of the eleven products that use the design system.
//!
//! A project publishes the packages its manifests name, unless they say they
//! are private. It uses a neighbour when a manifest of its own asks for one of
//! those by name. Only the files at the known places are read: the root, the
//! members a workspace names, and the usual directories of an app's second half.
//! Nothing is installed, and nothing is asked of a registry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// What a repository's manifests say about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifests {
    /// The packages it puts out for others to use.
    pub published: BTreeSet<String>,
    /// What it asks for from outside itself, with the version it asks for.
    pub used: BTreeMap<String, String>,
}

/// One project drawing on another, through a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Use {
    pub consumer: String,
    pub supplier: String,
    pub package: String,
    /// The version the consumer asks for, as its manifest spells it.
    pub requirement: String,
}

/// The directories an app keeps a second manifest in, beside the root.
const BESIDE: [&str; 6] = ["web", "gui", "ui", "app", "frontend", "src-tauri"];

/// Reads every manifest of a repository.
pub fn read(root: &Path) -> Manifests {
    let mut files = Vec::new();
    let mut members = Vec::new();
    let mut manifests = Manifests::default();

    for name in ["Cargo.toml", "package.json"] {
        files.push(root.join(name));
    }
    for dir in BESIDE {
        for name in ["Cargo.toml", "package.json"] {
            files.push(root.join(dir).join(name));
        }
    }
    if let Some(root_toml) = read_toml(&root.join("Cargo.toml")) {
        members.extend(string_list(root_toml.get("workspace").and_then(|w| w.get("members"))));
    }
    if let Some(root_json) = read_json(&root.join("package.json")) {
        match root_json.get("workspaces") {
            Some(serde_json::Value::Array(list)) => members.extend(list.iter().filter_map(|v| v.as_str().map(str::to_string))),
            Some(serde_json::Value::Object(map)) => {
                if let Some(serde_json::Value::Array(list)) = map.get("packages") {
                    members.extend(list.iter().filter_map(|v| v.as_str().map(str::to_string)));
                }
            }
            _ => {}
        }
    }
    members.extend(pnpm_packages(&root.join("pnpm-workspace.yaml")));
    for dir in members.iter().flat_map(|member| expand(root, member)) {
        for name in ["Cargo.toml", "package.json"] {
            files.push(dir.join(name));
        }
    }

    files.sort();
    files.dedup();
    for file in files {
        match file.file_name().and_then(|n| n.to_str()) {
            Some("Cargo.toml") => read_cargo(&file, &mut manifests),
            _ => read_package_json(&file, &mut manifests),
        }
    }
    // What it makes itself is no neighbour's.
    let own = manifests.published.clone();
    manifests.used.retain(|name, _| !own.contains(name));
    manifests
}

/// Who uses whom, across the projects given by name and directory.
///
/// A package two projects both publish is nobody's to claim, and left out rather
/// than guessed at: a wrong tie moves the wrong project.
pub fn uses(projects: &[(String, PathBuf)]) -> Vec<Use> {
    let read: Vec<(&String, Manifests)> = projects.iter().map(|(name, path)| (name, self::read(path))).collect();
    let mut publishers: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for (name, manifests) in &read {
        for package in &manifests.published {
            publishers.entry(package).or_default().push(name);
        }
    }
    let mut out = Vec::new();
    for (consumer, manifests) in &read {
        for (package, requirement) in &manifests.used {
            if let Some([supplier]) = publishers.get(package.as_str()).map(Vec::as_slice)
                && supplier != consumer
            {
                out.push(Use {
                    consumer: (*consumer).clone(),
                    supplier: (*supplier).clone(),
                    package: package.clone(),
                    requirement: requirement.clone(),
                });
            }
        }
    }
    out
}

fn read_toml(path: &Path) -> Option<toml::Value> {
    toml::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn string_list(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .map(|list| list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// The `packages:` list of a `pnpm-workspace.yaml`, read line by line: a
/// dependency on a YAML parser for one list of strings is not worth having.
fn pnpm_packages(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.trim_end() == "packages:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        match line.trim_start().strip_prefix("- ") {
            Some(item) if line.starts_with(' ') || line.starts_with('-') => {
                let item = item.split('#').next().unwrap_or(item).trim().trim_matches(['"', '\'']);
                if !item.starts_with('!') {
                    out.push(item.to_string());
                }
            }
            _ if line.trim().is_empty() || line.trim_start().starts_with('#') => {}
            _ => inside = false,
        }
    }
    out
}

/// A workspace member's directories: as named, or for `crates/*`, every directory
/// directly under it. No deeper pattern is used on this line, and one that is
/// not understood finds nothing rather than something wrong.
fn expand(root: &Path, member: &str) -> Vec<PathBuf> {
    match member.strip_suffix("/*") {
        None if member.contains('*') => Vec::new(),
        None => vec![root.join(member)],
        Some(parent) => std::fs::read_dir(root.join(parent))
            .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
            .unwrap_or_default(),
    }
}

fn read_cargo(path: &Path, out: &mut Manifests) {
    let Some(toml) = read_toml(path) else {
        return;
    };
    if let Some(package) = toml.get("package") {
        // `publish = false`, or an empty list of registries.
        let private = match package.get("publish") {
            Some(toml::Value::Boolean(publish)) => !publish,
            Some(toml::Value::Array(registries)) => registries.is_empty(),
            _ => false,
        };
        if !private && let Some(name) = package.get("name").and_then(toml::Value::as_str) {
            out.published.insert(name.to_string());
        }
    }
    let workspace_deps = toml.get("workspace").and_then(|w| w.get("dependencies"));
    let targets = toml.get("target").and_then(toml::Value::as_table);
    let mut tables: Vec<&toml::Value> = ["dependencies", "dev-dependencies", "build-dependencies"]
        .iter()
        .filter_map(|t| toml.get(*t))
        .collect();
    tables.extend(workspace_deps);
    for target in targets.into_iter().flat_map(|t| t.values()) {
        tables.extend(["dependencies", "dev-dependencies", "build-dependencies"].iter().filter_map(|t| target.get(*t)));
    }
    for (name, spec) in tables.iter().filter_map(|t| t.as_table()).flat_map(|t| t.iter()) {
        // A path with no version is the project's own, and `workspace = true`
        // is said once more at the workspace, where the version is.
        let requirement = match spec {
            toml::Value::String(version) => Some(version.as_str()),
            toml::Value::Table(table) => table.get("version").and_then(toml::Value::as_str),
            _ => None,
        };
        if let Some(requirement) = requirement {
            let name = spec.get("package").and_then(toml::Value::as_str).unwrap_or(name);
            out.used.entry(name.to_string()).or_insert_with(|| requirement.to_string());
        }
    }
}

fn read_package_json(path: &Path, out: &mut Manifests) {
    let Some(json) = read_json(path) else {
        return;
    };
    let private = json.get("private").and_then(serde_json::Value::as_bool).unwrap_or(false);
    if !private && let Some(name) = json.get("name").and_then(serde_json::Value::as_str) {
        out.published.insert(name.to_string());
    }
    for table in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        let Some(deps) = json.get(table).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for (name, requirement) in deps {
            let Some(requirement) = requirement.as_str() else {
                continue;
            };
            // The project's own packages, however they are spelt.
            if ["workspace:", "file:", "link:"].iter().any(|own| requirement.starts_with(own)) {
                continue;
            }
            out.used.entry(name.clone()).or_insert_with(|| requirement.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, file: &str, text: &str) {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_package_json_says_what_it_publishes_and_uses() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{ "name": "widget-ui", "dependencies": { "react": "^19.0.0", "own": "workspace:*" }, "devDependencies": { "vite": "8" } }"#,
        );
        let read = read(dir.path());
        assert_eq!(read.published, BTreeSet::from(["widget-ui".to_string()]));
        assert_eq!(read.used.get("react").map(String::as_str), Some("^19.0.0"));
        assert_eq!(read.used.get("vite").map(String::as_str), Some("8"));
        assert!(!read.used.contains_key("own"), "{read:?}");
    }

    #[test]
    fn a_private_package_publishes_nothing_but_still_uses() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{ "name": "app", "private": true, "dependencies": { "widget-ui": "^1" } }"#,
        );
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"app-core\"\npublish = false\n\n[dependencies]\nwidget-core = \"0.3\"\n",
        );
        let read = read(dir.path());
        assert!(read.published.is_empty(), "{read:?}");
        assert_eq!(read.used.len(), 2, "{read:?}");
    }

    /// The shapes this line's repositories come in: a Cargo workspace with its
    /// versions at the top, an app with its second half in a directory, npm and
    /// pnpm workspaces. A member's own crates are not its neighbours'.
    #[test]
    fn workspaces_and_second_halves_are_read() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\", \"tool\"]\n\n[workspace.dependencies]\nserde = \"1\"\ninner = { path = \"crates/inner\", version = \"0.2\" }\nwidget-core = { version = \"0.9\", features = [\"x\"] }\n",
        );
        write(
            dir.path(),
            "crates/inner/Cargo.toml",
            "[package]\nname = \"inner\"\n\n[dependencies]\nserde.workspace = true\n",
        );
        write(
            dir.path(),
            "tool/Cargo.toml",
            "[package]\nname = \"tool\"\npublish = false\n\n[target.'cfg(windows)'.dependencies]\nwindows-only = \"2\"\n",
        );
        write(
            dir.path(),
            "src-tauri/Cargo.toml",
            "[package]\nname = \"shell\"\npublish = []\n\n[dependencies]\ngadget-core = { package = \"gadget\", version = \"3\" }\n",
        );
        write(
            dir.path(),
            "web/package.json",
            r#"{ "name": "web", "private": true, "dependencies": { "widget-ui": "^0.4.0" } }"#,
        );
        let read = read(dir.path());
        assert_eq!(read.published, BTreeSet::from(["inner".to_string()]), "{read:?}");
        for (package, requirement) in [
            ("serde", "1"),
            ("widget-core", "0.9"),
            ("windows-only", "2"),
            ("gadget", "3"),
            ("widget-ui", "^0.4.0"),
        ] {
            assert_eq!(read.used.get(package).map(String::as_str), Some(requirement), "{package}: {read:?}");
        }
        assert!(!read.used.contains_key("inner"), "its own crate: {read:?}");
    }

    #[test]
    fn a_pnpm_workspace_names_its_members() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "pnpm-workspace.yaml",
            "packages:\n  - 'packages/*'\n  # a note\n  - \"site\"\n  - '!packages/skip'\nverifyDepsBeforeRun: install\n",
        );
        write(
            dir.path(),
            "packages/ui/package.json",
            r#"{ "name": "widget-ui", "dependencies": { "react": "19" } }"#,
        );
        write(
            dir.path(),
            "site/package.json",
            r#"{ "name": "site", "private": true, "dependencies": { "astro": "7" } }"#,
        );
        let read = read(dir.path());
        assert_eq!(read.published, BTreeSet::from(["widget-ui".to_string()]), "{read:?}");
        assert!(read.used.contains_key("react") && read.used.contains_key("astro"), "{read:?}");
    }

    #[test]
    fn a_broken_or_missing_manifest_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", "{ not json");
        write(dir.path(), "Cargo.toml", "not = [toml");
        assert_eq!(read(dir.path()), Manifests::default());
        assert_eq!(read(&dir.path().join("nowhere")), Manifests::default());
    }

    #[test]
    fn who_uses_whom_follows_the_packages() {
        let dir = tempfile::tempdir().unwrap();
        let path = |name: &str| dir.path().join(name);
        write(&path("ui"), "package.json", r#"{ "name": "widget-ui" }"#);
        write(
            &path("app"),
            "package.json",
            r#"{ "name": "app", "private": true, "dependencies": { "widget-ui": "^1.2.0", "react": "19" } }"#,
        );
        write(
            &path("other"),
            "package.json",
            r#"{ "name": "other", "private": true, "dependencies": { "react": "19" } }"#,
        );
        write(&path("twin-a"), "package.json", r#"{ "name": "twin" }"#);
        write(&path("twin-b"), "package.json", r#"{ "name": "twin" }"#);
        write(
            &path("user"),
            "package.json",
            r#"{ "name": "user", "private": true, "dependencies": { "twin": "1" } }"#,
        );
        let projects: Vec<(String, PathBuf)> = ["ui", "app", "other", "twin-a", "twin-b", "user"]
            .iter()
            .map(|n| (n.to_string(), path(n)))
            .collect();
        assert_eq!(
            uses(&projects),
            vec![Use {
                consumer: "app".to_string(),
                supplier: "ui".to_string(),
                package: "widget-ui".to_string(),
                requirement: "^1.2.0".to_string(),
            }]
        );
    }
}
