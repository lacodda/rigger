//! Commit messages as facts.
//!
//! The line writes Conventional Commits, so a commit already says what kind
//! of change it is. Reading that is cheaper than writing the same sentence
//! twice - once in the commit and once in a note afterwards - and it means
//! the record stays current without anyone opening a session.
//!
//! Not every commit is a change to the product. `chore`, `docs`, `test`,
//! `refactor` and the rest are how the work was done, not what shipped; they
//! belong in git, where they already are. Only `feat`, `fix` and anything
//! marked breaking become events (the owner's rule, and the one a changelog
//! generator uses).

/// A commit message that follows the convention.
#[derive(Debug, PartialEq, Eq)]
pub struct Conventional {
    pub kind: String,
    pub scope: Option<String>,
    pub breaking: bool,
    pub summary: String,
}

/// The types worth recording as a change to the product.
const RECORDED: [&str; 2] = ["feat", "fix"];

impl Conventional {
    /// Whether this commit changed the product, rather than the work around
    /// it. A breaking commit is recorded whatever its type: `refactor!` that
    /// removes a flag is a change to the product by any reading.
    pub fn is_a_change(&self) -> bool {
        self.breaking || RECORDED.contains(&self.kind.as_str())
    }

    /// The event's text: the summary, with what the convention says about it.
    pub fn body(&self) -> String {
        let mut out = String::new();
        if self.breaking {
            out.push_str("breaking: ");
        }
        if let Some(scope) = &self.scope {
            out.push_str(&format!("{}({scope}): ", self.kind));
        } else {
            out.push_str(&format!("{}: ", self.kind));
        }
        out.push_str(&self.summary);
        out
    }
}

/// Reads the first line of a commit message, and the rest only for the
/// footer that marks a break.
///
/// Returns `None` for a message that does not follow the convention - an
/// older repository, a merge, a commit written before the line adopted it.
/// Those are not errors: they are simply not facts this can read.
pub fn parse(message: &str) -> Option<Conventional> {
    let mut lines = message.lines();
    let subject = lines.next()?.trim();

    let (head, summary) = subject.split_once(": ")?;
    let summary = summary.trim();
    if summary.is_empty() {
        return None;
    }

    // `type`, `type(scope)`, `type!` or `type(scope)!`.
    let (head, breaking_marker) = match head.strip_suffix('!') {
        Some(head) => (head, true),
        None => (head, false),
    };
    let (kind, scope) = match head.split_once('(') {
        Some((kind, rest)) => (kind, Some(rest.strip_suffix(')')?)),
        None => (head, None),
    };
    if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    if scope.is_some_and(|s| s.is_empty()) {
        return None;
    }

    // The other half of the convention: a `BREAKING CHANGE:` footer says the
    // same thing as `!`, and a commit may use either.
    let breaking = breaking_marker
        || message
            .lines()
            .skip(1)
            .any(|l| l.starts_with("BREAKING CHANGE:") || l.starts_with("BREAKING-CHANGE:"));

    Some(Conventional {
        kind: kind.to_ascii_lowercase(),
        scope: scope.map(str::to_string),
        breaking,
        summary: summary.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(message: &str) -> Conventional {
        parse(message).unwrap_or_else(|| panic!("did not parse: {message:?}"))
    }

    #[test]
    fn a_type_and_a_summary_are_enough() {
        let c = parsed("feat: read tags and commits into facts");
        assert_eq!(c.kind, "feat");
        assert_eq!(c.scope, None);
        assert!(!c.breaking);
        assert_eq!(c.summary, "read tags and commits into facts");
    }

    #[test]
    fn a_scope_travels_with_the_type() {
        let c = parsed("feat(open): start a session with the packet ready");
        assert_eq!(c.kind, "feat");
        assert_eq!(c.scope.as_deref(), Some("open"));
        assert_eq!(c.body(), "feat(open): start a session with the packet ready");
    }

    #[test]
    fn a_break_is_marked_either_way() {
        // Both spellings the convention allows.
        assert!(parsed("feat!: drop the old flag").breaking);
        assert!(parsed("feat(cli)!: drop the old flag").breaking);
        assert!(parsed("feat: drop the old flag\n\nBREAKING CHANGE: the flag is gone").breaking);
        assert!(!parsed("feat: keep the old flag").breaking);
    }

    #[test]
    fn only_changes_to_the_product_are_recorded() {
        // The owner's rule: what shipped, not how the work was done.
        assert!(parsed("feat: a new thing").is_a_change());
        assert!(parsed("fix: a broken thing").is_a_change());
        for noise in [
            "chore(release): v0.6.0",
            "docs: changelog for v0.6.0",
            "test: read past the byte-order mark",
            "ci: publish to the registries on the tag",
            "refactor: rename a private function",
            "style: reformat",
        ] {
            assert!(!parsed(noise).is_a_change(), "{noise} is not a change to the product");
        }
    }

    #[test]
    fn a_breaking_commit_is_recorded_whatever_its_type() {
        // A refactor that removes a flag is a change to the product by any
        // reading, whatever the author called it.
        let c = parsed("refactor!: remove the deprecated flag");
        assert!(c.is_a_change());
        assert!(c.body().starts_with("breaking: "), "{}", c.body());
    }

    #[test]
    fn a_message_that_does_not_follow_the_convention_is_not_a_fact() {
        // Older repositories in this line predate the convention; those
        // commits are history, not facts this can read.
        assert_eq!(parse("Initial commit"), None);
        assert_eq!(parse("Merge branch 'main' into feature"), None);
        assert_eq!(parse("feat:no space after the colon"), None);
        assert_eq!(parse("feat: "), None);
        assert_eq!(parse(""), None);
        // A colon in prose is not a type.
        assert_eq!(parse("update the readme: it was stale"), None);
        assert_eq!(parse("v1.2: release"), None);
    }

    #[test]
    fn an_unclosed_scope_is_not_a_fact() {
        assert_eq!(parse("feat(open: start a session"), None);
        assert_eq!(parse("feat(): start a session"), None);
    }

    #[test]
    fn the_type_is_read_case_insensitively() {
        // Written by hand often enough to be worth reading.
        assert_eq!(parsed("Fix: a broken thing").kind, "fix");
    }
}
