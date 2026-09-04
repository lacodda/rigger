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

#[cfg(test)]
mod tests {
    use super::*;

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
