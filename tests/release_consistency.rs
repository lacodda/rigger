//! Guards the facts that must agree before a version is published.
//!
//! rigger ships to three places - GitHub, crates.io and npm - each of which
//! renders its own copy of the metadata. They drift silently: nothing fails
//! when `npm/package.json` still says 0.1.0, or when the npm page describes
//! the product differently from the crate. The drift is only visible after
//! publishing, when it is too late to take back.
//!
//! These checks run in CI, so a mismatch fails the build instead of shipping.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn read(path: impl AsRef<Path>) -> String {
        let path = repo_root().join(path);
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
    }

    /// Extracts a top-level `key = "value"` from Cargo.toml.
    ///
    /// Deliberately naive: it reads only the `[package]` block, which is all
    /// these checks need, and avoids adding a TOML parser as a dev-dependency.
    fn cargo_field(key: &str) -> String {
        let manifest = read("Cargo.toml");
        for line in manifest.lines() {
            let line = line.trim();
            // Stop at the next section: `version` also appears under [lib],
            // [[bin]] and in every dependency.
            if line.starts_with('[') && line != "[package]" {
                break;
            }
            let Some((name, value)) = line.split_once('=') else { continue };
            // Exact match, so `rust-version` cannot answer a lookup for `version`.
            if name.trim() != key {
                continue;
            }
            return value.trim().trim_matches('"').to_string();
        }
        panic!("`{key}` not found in the [package] block of Cargo.toml");
    }

    /// Extracts a `"key": "value"` from a JSON file, without a JSON dependency.
    fn json_field(file: &str, key: &str) -> String {
        let text = read(file);
        let needle = format!("\"{key}\"");
        let start = text.find(&needle).unwrap_or_else(|| panic!("`{key}` not found in {file}"));
        let after = &text[start + needle.len()..];
        let after = after.trim_start().trim_start_matches(':').trim_start();
        let after = after.strip_prefix('"').unwrap_or_else(|| panic!("`{key}` in {file} is not a string"));
        after[..after.find('"').expect("unterminated string")].to_string()
    }

    #[test]
    fn npm_package_version_matches_the_crate() {
        let crate_version = cargo_field("version");
        let npm_version = json_field("npm/package.json", "version");

        assert_eq!(
            npm_version, crate_version,
            "npm/package.json version ({npm_version}) differs from Cargo.toml ({crate_version}); \
             the npm page would advertise a version that was never released"
        );
    }

    #[test]
    fn npm_wrapper_downloads_the_matching_binary() {
        let crate_version = cargo_field("version");
        let binary_tag = json_field("npm/package.json", "binary");

        assert_eq!(
            binary_tag,
            format!("v{crate_version}"),
            "the npm wrapper points at release {binary_tag} while this is {crate_version}; \
             installing from npm would fetch the wrong binary"
        );
    }

    #[test]
    fn the_product_is_described_the_same_way_everywhere() {
        let crate_description = cargo_field("description");
        let npm_description = json_field("npm/package.json", "description");

        assert_eq!(
            npm_description, crate_description,
            "crates.io and npm describe the product differently; \
             Cargo.toml `description` is the single source"
        );
    }

    #[test]
    fn readme_is_shared_rather_than_duplicated() {
        // A second copy under npm/ is what let the two pages drift apart. The
        // npm package takes the root README at publish time instead.
        let duplicate = repo_root().join("npm/README.md");
        assert!(
            !duplicate.exists(),
            "npm/README.md exists again; it will drift from the root README. \
             The publish workflow copies the root one into npm/ instead."
        );
    }

    #[test]
    fn readme_links_resolve_off_github() {
        // The same file is rendered on crates.io and npm, where a relative
        // path has no repository to resolve against: the banner turns into a
        // broken image and the links 404.
        let readme = read("README.md");

        for (line_no, line) in readme.lines().enumerate() {
            for (marker, kind) in [("src=\"", "image"), ("](", "link")] {
                let mut rest = line;
                while let Some(at) = rest.find(marker) {
                    let target = &rest[at + marker.len()..];
                    let end = if marker == "](" { ')' } else { '"' };
                    let target = &target[..target.find(end).unwrap_or(target.len())];

                    let relative = !target.starts_with("http") && !target.starts_with('#') && !target.is_empty();
                    assert!(
                        !relative,
                        "README line {}: relative {kind} `{target}` breaks on crates.io and npm; use an absolute URL",
                        line_no + 1
                    );

                    rest = &rest[at + marker.len()..];
                }
            }
        }
    }

    #[test]
    fn the_unix_installer_redirects_windows_shells() {
        // Field report from kasl, 19.08: run in Git Bash on Windows, the
        // script matched no case arm and answered "No prebuilt binary for
        // MINGW64_NT-…", which reads as "unsupported platform" although a
        // Windows release exists - it is just installed by the other script.
        let installer = read("tools/install.sh");

        for shell in ["MINGW*", "MSYS*", "CYGWIN*"] {
            assert!(
                installer.contains(shell),
                "install.sh does not recognise {shell}; Windows shells fall through to the generic 'no prebuilt binary' message"
            );
        }
        assert!(
            installer.contains("install.ps1"),
            "install.sh does not name the PowerShell installer, leaving Windows users at a dead end"
        );
    }

    #[test]
    fn installers_name_the_crate_that_actually_exists() {
        // A `cargo install <name>` fallback that does not resolve is worse
        // than none: in kasl the suggestion named `kasl` while the crate is
        // published as `kasl-cli`, so the advice in the error message failed.
        let crate_name = cargo_field("name");

        for file in ["tools/install.sh", "tools/install.ps1", "npm/download.js"] {
            let text = read(file);
            for (line_no, line) in text.lines().enumerate() {
                let Some(at) = line.find("cargo install ") else { continue };
                // Trim shell quoting around the suggestion, e.g. `... rigger" >&2`.
                let named = line[at + "cargo install ".len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
                assert_eq!(
                    named,
                    crate_name,
                    "{file} line {} suggests `cargo install {named}`, but the crate is `{crate_name}`",
                    line_no + 1
                );
            }
        }
    }

    /// The installers must ask for release assets that release.yml actually
    /// builds. kasl's installer requested targets that never existed and
    /// answered 404 on every Unix machine for months.
    #[test]
    fn installers_ask_for_targets_the_release_builds() {
        let workflow = read(".github/workflows/release.yml");
        for (file, targets) in [
            ("tools/install.sh", vec!["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]),
            ("tools/install.ps1", vec!["x86_64-pc-windows-msvc"]),
            (
                "npm/download.js",
                vec!["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "aarch64-apple-darwin"],
            ),
        ] {
            let text = read(file);
            for target in targets {
                assert!(text.contains(target), "{file} no longer names {target}");
                assert!(workflow.contains(target), "{file} asks for {target}, which release.yml does not build");
            }
        }
    }

    #[test]
    fn readme_only_shows_commands_that_exist() {
        // Every `$ rigger <word>` in a console block must name a real
        // subcommand. A README outlives the surface it documents: kasl's
        // advertised a removed command for months before anyone noticed.
        let readme = read("README.md");
        let help = String::from_utf8(
            std::process::Command::new(env!("CARGO_BIN_EXE_rigger"))
                .arg("--help")
                .output()
                .expect("cannot run rigger --help")
                .stdout,
        )
        .expect("help output is not utf-8");

        // Subcommand names are the indented first words in the Commands block.
        let known: Vec<String> = help
            .lines()
            .skip_while(|l| !l.starts_with("Commands:"))
            .skip(1)
            .take_while(|l| l.starts_with("  ") && !l.trim().is_empty())
            .filter_map(|l| l.split_whitespace().next())
            .map(str::to_string)
            .collect();

        for line in readme.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("$ rigger ") else { continue };
            let Some(word) = rest.split_whitespace().next() else { continue };
            if word.starts_with('-') {
                continue; // a flag on the bare binary, e.g. `rigger --version`
            }
            assert!(
                known.contains(&word.to_string()),
                "README shows `rigger {word}`, which is not a command; known: {known:?}"
            );
        }
    }

    /// The docs site and the repository must show the same mark at each level:
    /// the header logo is the L tile, the favicon the S tile. Swapping them is a
    /// real mistake that has happened in the line - an S tile in a 40px header
    /// reads as a solid coloured block.
    #[test]
    fn the_docs_site_carries_the_same_mark_as_the_repository() {
        for (site, repo) in [
            ("docs/src/assets/logo.svg", "assets/logo.svg"),
            ("docs/public/favicon.svg", "assets/logo-s.svg"),
            ("docs/public/apple-touch-icon.png", "assets/apple-touch-icon.png"),
        ] {
            let a = fs::read(repo_root().join(site)).unwrap_or_else(|e| panic!("cannot read {site}: {e}"));
            let b = fs::read(repo_root().join(repo)).unwrap_or_else(|e| panic!("cannot read {repo}: {e}"));
            assert!(a == b, "{site} differs from {repo}; the docs site shows a different mark than the repository");
        }
    }

    /// The mark is a pair of colours drawn as one gradient. A gradient in
    /// `objectBoundingBox` units does not render on a straight line at all (the
    /// box has no area), so the masters use tile coordinates; a regenerated
    /// asset that lost that would silently drop every line of the metaphor.
    #[test]
    fn the_svg_tiles_paint_the_pair_in_tile_coordinates() {
        for file in ["assets/logo.svg", "assets/logo-m.svg", "assets/logo-s.svg", "assets/banner.svg"] {
            let svg = read(file);
            assert!(svg.contains("gradientUnits=\"userSpaceOnUse\""), "{file} does not paint the pair in tile coordinates");
            for colour in ["#8A62F0", "#2FB3C6"] {
                assert!(svg.contains(colour), "{file} lacks the pair colour {colour}");
            }
        }
    }

    /// The Windows icon carries one image per size, largest first: some
    /// readers take the first directory entry verbatim as the window icon, and
    /// a 16px first entry gives a title bar stretched from sixteen pixels.
    #[test]
    fn the_ico_lists_its_images_largest_first() {
        let ico = fs::read(repo_root().join("assets/icon.ico")).expect("cannot read assets/icon.ico");
        assert!(ico.len() > 6, "icon.ico is truncated");
        assert_eq!(u16::from_le_bytes([ico[2], ico[3]]), 1, "icon.ico is not an icon resource");
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        let sizes: Vec<u32> = (0..count)
            .map(|i| {
                let width = ico[6 + 16 * i];
                if width == 0 { 256 } else { u32::from(width) }
            })
            .collect();
        assert_eq!(sizes, vec![256, 128, 64, 48, 32, 24, 16], "icon.ico entries are {sizes:?}; expected every level, largest first");
    }

    /// A version number in README prose goes stale the moment the next
    /// release ships. Checked here rather than by eye: the doc sweep at the
    /// end of a stage is the step most likely to be rushed.
    #[test]
    fn the_readme_sample_shows_the_current_version() {
        let readme = read("README.md");
        let version = cargo_field("version");
        let stale: Vec<&str> = readme
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("rigger 0.") && !line.contains(&version))
            .collect();
        assert!(
            stale.is_empty(),
            "the README shows an old version in a sample: {stale:?} (this release is {version})"
        );
    }
}
