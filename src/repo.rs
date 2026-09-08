//! What a repository says about itself: its name and its origin remote.
//!
//! The name is the directory name. A manifest often declares something else -
//! the crate `kasl-cli` for the product `kasl`, a `*-workspace` root for a
//! monorepo - and that is the publishing name, not the one the owner uses in
//! conversation. `--name` overrides when the directory is not it either.
//!
//! The remote is read from `.git/config` directly; reading history and tags
//! arrives with the sync release and goes through gix, never through a
//! spawned `git`.

use std::path::Path;

/// The project name: the repository's directory name.
pub fn detect_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string())
}

/// The URL of the `origin` remote, if the repository has one.
pub fn detect_remote(root: &Path) -> Option<String> {
    let config = git_config(root)?;
    let mut in_origin = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_origin = line == "[remote \"origin\"]";
            continue;
        }
        if !in_origin {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == "url"
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// `.git` is a directory in a plain checkout and a pointer file in a
/// worktree; the config lives next to the real git directory either way.
fn git_config(root: &Path) -> Option<String> {
    let dot_git = root.join(".git");
    let git_dir = if dot_git.is_dir() {
        dot_git
    } else {
        let pointer = std::fs::read_to_string(&dot_git).ok()?;
        let target = pointer.trim().strip_prefix("gitdir:")?.trim();
        let target = Path::new(target);
        if target.is_absolute() { target.to_path_buf() } else { root.join(target) }
    };
    std::fs::read_to_string(git_dir.join("config")).ok()
}

/// One line about the project, from the manifest that publishes it.
///
/// The `description` of a `Cargo.toml` or a `package.json`, whichever the
/// root holds - written once by the owner for the registry, and good enough
/// for a skill's front matter to say what the name stands for. Nothing is
/// guessed when neither says anything.
pub fn detect_about(root: &Path) -> Option<String> {
    if let Ok(text) = std::fs::read_to_string(root.join("Cargo.toml")) {
        let mut in_package = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_package = line == "[package]";
                continue;
            }
            if in_package
                && let Some((key, value)) = line.split_once('=')
                && key.trim() == "description"
            {
                let value = value.trim().trim_matches('"').trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string(root.join("package.json"))
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(about) = json.get("description").and_then(|d| d.as_str())
        && !about.trim().is_empty()
    {
        return Some(about.trim().to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_comes_from_the_package_section_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\ndescription = \"A thing\"\n[dependencies]\ndescription = \"not this\"\n",
        )
        .unwrap();
        assert_eq!(detect_about(dir.path()).as_deref(), Some("A thing"));
    }

    #[test]
    fn about_falls_back_to_package_json_then_to_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_about(dir.path()), None);
        std::fs::write(dir.path().join("package.json"), "{\"name\":\"x\",\"description\":\"From npm\"}").unwrap();
        assert_eq!(detect_about(dir.path()).as_deref(), Some("From npm"));
    }

    #[test]
    fn the_name_is_the_directory_name_whatever_the_manifest_says() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("product");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"product-cli\"\n").unwrap();
        assert_eq!(detect_name(&root), "product");
    }

    #[test]
    fn remote_is_read_from_the_origin_section_only() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git");
        std::fs::create_dir(&git).unwrap();
        std::fs::write(
            git.join("config"),
            "[core]\n\tbare = false\n[remote \"upstream\"]\n\turl = https://example.com/upstream.git\n[remote \"origin\"]\n\turl = https://example.com/origin.git\n",
        )
        .unwrap();
        assert_eq!(detect_remote(dir.path()).as_deref(), Some("https://example.com/origin.git"));
    }

    #[test]
    fn a_worktree_pointer_file_is_followed() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real-git");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("config"), "[remote \"origin\"]\n\turl = git@example.com:o/r.git\n").unwrap();
        let tree = dir.path().join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        assert_eq!(detect_remote(&tree).as_deref(), Some("git@example.com:o/r.git"));
    }

    #[test]
    fn no_git_means_no_remote() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_remote(dir.path()), None);
    }
}
