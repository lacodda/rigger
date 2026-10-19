//! What a repository says about itself: its name and its origin remote.
//!
//! The name is the directory name. A manifest often declares something else -
//! the crate `kasl-cli` for the product `kasl`, a `*-workspace` root for a
//! monorepo - and that is the publishing name, not the one the owner uses in
//! conversation. `--name` overrides when the directory is not it either.
//!
//! The remote is read from `.git/config` directly; history and tags are read
//! through furca-core (see `git`), never through a spawned `git`.

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

/// The one line about a project that is in force: its manifest's, else the
/// one the record holds.
///
/// Every screen that names what a project is reads it here - the skill, the
/// line's skill, `show`, the registry - so that a project described in two
/// places cannot be described two ways. The record's line is written only
/// where the manifest is silent (`project set --about` refuses otherwise),
/// and the manifest still wins should it gain a description later.
pub fn about(project: &crate::db::Project) -> Option<String> {
    let manifest = if project.kind.has_a_place() {
        detect_about(Path::new(&project.path))
    } else {
        None
    };
    manifest.or_else(|| project.about.clone())
}

/// One line about the project, from the manifest that publishes it.
///
/// The `description` of a `Cargo.toml` or a `package.json`, whichever the
/// root holds - written once by the owner for the registry, and good enough
/// for a skill's front matter to say what the name stands for. Nothing is
/// guessed when neither says anything.
pub fn detect_about(root: &Path) -> Option<String> {
    if let Ok(text) = std::fs::read_to_string(root.join("Cargo.toml")) {
        if let Some(about) = section_description(&text, "[package]") {
            return Some(about);
        }
        // A workspace states what its members share in `[workspace.package]`,
        // and that is where a product split into crates keeps its own line:
        // members inherit it with `description.workspace = true`. Read after
        // the root's own package and before any member, because a member
        // speaks for its crate - efema came back as "The wire format of
        // efema", true of the first member and false of the product.
        if let Some(about) = section_description(&text, "[workspace.package]") {
            return Some(about);
        }
        // A workspace root has no `[package]` of its own, and the product's
        // description lives in one of its members. Reading only the root
        // gave five projects of this line no description at all - in the
        // skill an assistant reads, in the public registry, and on the
        // project's own screen - and nothing said so, because a project
        // genuinely without a description looks exactly the same.
        if let Some(about) = workspace_description(root, &text) {
            return Some(about);
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

/// The `description` a manifest states in one section, such as its own
/// `[package]` or the `[workspace.package]` its members share.
///
/// Only a written string counts. A member that inherits - spelt
/// `description.workspace = true` or `description = { workspace = true }` -
/// says nothing of its own rather than something wrong; the second form
/// came back as the text `{ workspace = true }` before strings were asked
/// for.
fn section_description(text: &str, section: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if in_section
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "description"
        {
            let value = value.trim();
            let Some(quote) = value.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                continue;
            };
            let value = value.trim_matches(quote).trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// The description of the member that is the product.
///
/// A workspace splits a product into crates, and only one of them is the
/// product - the rest are the library it is built on and the plugins it
/// loads. Their descriptions describe those parts, so picking the wrong
/// member does not say "no description": it says something confidently
/// wrong. sefy came back as "Core library for sefy", which is true of the
/// crate and false of the product.
///
/// So the member is chosen, in order: the one named after the project,
/// then the one named `<project>-cli`, which is how this line names the
/// crate that ships. Only if neither is there does it fall back to the
/// first member that describes itself - a guess, but a guess in a
/// workspace that has not said which crate is the product.
fn workspace_description(root: &Path, text: &str) -> Option<String> {
    let members = workspace_members(text);
    if members.is_empty() {
        return None;
    }
    let name = root.file_name()?.to_string_lossy().to_string();
    let described = |member: &String| -> Option<String> {
        let text = std::fs::read_to_string(root.join(member).join("Cargo.toml")).ok()?;
        section_description(&text, "[package]")
    };
    let basename = |member: &String| member.rsplit(['/', '\\']).next().unwrap_or(member).to_string();
    for wanted in [name.clone(), format!("{name}-cli")] {
        if let Some(about) = members.iter().find(|m| basename(m) == wanted).and_then(described) {
            return Some(about);
        }
    }
    members.iter().find_map(&described)
}

/// The paths a `[workspace]` lists as members.
///
/// Read by hand rather than with a TOML parser, as the rest of this module
/// is: the list is written on one line or over many, and both forms are in
/// this line's repositories.
fn workspace_members(text: &str) -> Vec<String> {
    let mut members = Vec::new();
    let mut in_workspace = false;
    let mut in_list = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && !in_list {
            in_workspace = line == "[workspace]";
            continue;
        }
        if !in_workspace {
            continue;
        }
        let rest = match line.split_once('=') {
            Some((key, rest)) if key.trim() == "members" => {
                in_list = true;
                rest.trim()
            }
            _ if in_list => line,
            _ => continue,
        };
        for piece in rest.trim_start_matches('[').trim_end_matches(']').split(',') {
            let piece = piece.trim().trim_matches('"').trim();
            if !piece.is_empty() {
                members.push(piece.to_string());
            }
        }
        if rest.contains(']') {
            in_list = false;
        }
    }
    members
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five projects of this line are workspaces whose root states no
    /// description, and every one came back as "a project recorded in
    /// rigger". Nothing said so, because a project genuinely without a
    /// description looks the same.
    #[test]
    fn a_workspace_is_described_by_the_member_that_is_the_product() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("nooma");
        std::fs::create_dir_all(root.join("crates").join("nooma")).unwrap();
        std::fs::create_dir_all(root.join("crates").join("nooma-core")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/nooma\", \"crates/nooma-core\"]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"0.2.0\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates").join("nooma").join("Cargo.toml"),
            "[package]\nname = \"nooma\"\ndescription = \"Local semantic search\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates").join("nooma-core").join("Cargo.toml"),
            "[package]\nname = \"nooma-core\"\ndescription = \"The library behind it\"\n",
        )
        .unwrap();

        assert_eq!(detect_about(&root).as_deref(), Some("Local semantic search"));
    }

    /// The crate that ships wins over the library it is built on, however
    /// the list is ordered - and the list is read whether it was written
    /// on one line or over many, both of which are in this line's
    /// repositories.
    ///
    /// Picking the wrong member is worse than picking none: sefy came back
    /// as "Core library for sefy", which is true of the crate and false of
    /// the product, and reads as an answer rather than as a gap.
    #[test]
    fn the_crate_that_ships_wins_over_the_library_it_sits_on() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sefy");
        for member in ["sefy-core", "sefy-cli"] {
            std::fs::create_dir_all(root.join("crates").join(member)).unwrap();
        }
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\n    \"crates/sefy-core\",\n    \"crates/sefy-cli\",\n]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates").join("sefy-core").join("Cargo.toml"),
            "[package]\nname = \"sefy-core\"\ndescription = \"Core library for sefy\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates").join("sefy-cli").join("Cargo.toml"),
            "[package]\nname = \"sefy-cli\"\ndescription = \"An inconspicuous encrypted store\"\n",
        )
        .unwrap();
        assert_eq!(detect_about(&root).as_deref(), Some("An inconspicuous encrypted store"));

        // And a member named exactly after the project outranks even that.
        let named = dir.path().join("nooma");
        for member in ["nooma-cli", "nooma"] {
            std::fs::create_dir_all(named.join("crates").join(member)).unwrap();
        }
        std::fs::write(named.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/nooma-cli\", \"crates/nooma\"]\n").unwrap();
        std::fs::write(
            named.join("crates").join("nooma-cli").join("Cargo.toml"),
            "[package]\nname = \"nooma-cli\"\ndescription = \"The wrapper\"\n",
        )
        .unwrap();
        std::fs::write(
            named.join("crates").join("nooma").join("Cargo.toml"),
            "[package]\nname = \"nooma\"\ndescription = \"The product\"\n",
        )
        .unwrap();
        assert_eq!(detect_about(&named).as_deref(), Some("The product"));
    }

    /// A workspace whose product is named otherwise still says something:
    /// the one member that describes itself is the answer.
    #[test]
    fn a_workspace_with_one_described_member_means_that_one() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("austeris");
        std::fs::create_dir_all(root.join("crates").join("api")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/api\"]\n").unwrap();
        std::fs::write(
            root.join("crates").join("api").join("Cargo.toml"),
            "[package]\nname = \"api\"\ndescription = \"Household bookkeeping\"\n",
        )
        .unwrap();
        assert_eq!(detect_about(&root).as_deref(), Some("Household bookkeeping"));

        // A workspace whose members say nothing is still nothing, rather
        // than an error or an invented line.
        let bare = dir.path().join("bare");
        std::fs::create_dir_all(bare.join("crates").join("x")).unwrap();
        std::fs::write(bare.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/x\"]\n").unwrap();
        std::fs::write(bare.join("crates").join("x").join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert_eq!(detect_about(&bare), None);
    }

    /// A workspace that keeps the product's line in `[workspace.package]`
    /// is described by it, ahead of any member: the first member speaks for
    /// its own crate. Before, the members were read and the first one that
    /// described itself won - "the wire format", true of a crate and false
    /// of the product - while the member that inherits the workspace's line
    /// was passed over as silent.
    #[test]
    fn the_workspace_package_speaks_for_the_product_before_any_member() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("relay");
        for member in ["relay-proto", "relay-server"] {
            std::fs::create_dir_all(root.join("crates").join(member)).unwrap();
        }
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/relay-proto\", \"crates/relay-server\"]\n\n[workspace.package]\nversion = \"0.5.0\"\ndescription = \"A sync relay\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates").join("relay-proto").join("Cargo.toml"),
            "[package]\nname = \"relay-proto\"\ndescription = \"The wire format of relay\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates").join("relay-server").join("Cargo.toml"),
            "[package]\nname = \"relay-server\"\ndescription.workspace = true\n",
        )
        .unwrap();
        assert_eq!(detect_about(&root).as_deref(), Some("A sync relay"));

        // The root's own package still comes first.
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"relay\"\ndescription = \"The root\"\n\n[workspace]\nmembers = [\"crates/relay-proto\"]\n\n[workspace.package]\ndescription = \"A sync relay\"\n",
        )
        .unwrap();
        assert_eq!(detect_about(&root).as_deref(), Some("The root"));
    }

    /// An inherited description, in either spelling, is not a description:
    /// the inline-table form used to come back as its own text.
    #[test]
    fn an_inherited_description_says_nothing_rather_than_its_own_syntax() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("relay");
        std::fs::create_dir_all(root.join("crates").join("relay")).unwrap();
        std::fs::create_dir_all(root.join("crates").join("relay-core")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/relay\", \"crates/relay-core\"]\n").unwrap();
        std::fs::write(
            root.join("crates").join("relay").join("Cargo.toml"),
            "[package]\nname = \"relay\"\ndescription = { workspace = true }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates").join("relay-core").join("Cargo.toml"),
            "[package]\nname = \"relay-core\"\ndescription = 'Core of relay'\n",
        )
        .unwrap();
        assert_eq!(detect_about(&root).as_deref(), Some("Core of relay"));
    }

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
