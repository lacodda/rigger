//! The line as the world sees it: a public registry, written from the record.
//!
//! Every generator on this line needs the same list - which products exist,
//! what each one's mark and colour are, what shape of thing it is, where to
//! read about it. Until now each one carried its own copy: a table in a
//! notes vault, a constant in `lyrn` that `lyrn new` reads, whatever a
//! README happened to say. On the day this was written lyrn's copy was five
//! products behind, and nothing could have said so - a hardcoded list does
//! not know it is stale.
//!
//! So the record holds it once and publishes it. What is published is only
//! what is already public: a name, a mark, a colour, a repository, a
//! documentation site. Never a path on a machine, never a hub, never a
//! stage, never a question waiting for the owner. That boundary is the
//! whole risk of this command, so it is a test rather than a habit.

use anyhow::{Result, bail};
use serde::Serialize;

use crate::db::{Db, Kind, Project};

/// The shapes of thing this line builds.
///
/// A closed list rather than free text: a generator switches on it, and a
/// form spelt two ways is a branch that silently does not run.
pub const FORMS: [&str; 5] = ["cli", "desktop", "web", "library", "service"];

/// The whole registry, as it is published.
#[derive(Debug, Serialize)]
pub struct Registry {
    /// What wrote this, so a consumer can tell a hand-edited file from one
    /// that will be overwritten.
    pub generator: &'static str,
    pub version: &'static str,
    pub products: Vec<Product>,
}

/// One product, as the world may see it.
#[derive(Debug, Serialize)]
pub struct Product {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// The second colour, for a mark drawn as a pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
}

/// Builds the registry from the record.
///
/// A project with no repository is left out: the registry says what the
/// line ships, and a place the record keeps for itself ships nothing.
pub fn build(db: &Db, about: impl Fn(&Project) -> Option<String>) -> Result<Registry> {
    let mut products: Vec<Product> = db
        .projects()?
        .into_iter()
        .filter(|p| p.kind == Kind::Repo)
        .map(|p| Product {
            about: about(&p),
            name: p.name,
            mark: p.mark_code,
            accent: p.accent,
            accent2: p.accent2,
            form: p.form,
            repository: p.remote,
            docs: p.docs_url,
        })
        .collect();
    products.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Registry {
        generator: "rigger",
        version: env!("CARGO_PKG_VERSION"),
        products,
    })
}

/// Whether anything private got into the registry.
///
/// Run before the file is written, every time, not as a review someone
/// remembers to do. The record holds paths on the owner's machine, the
/// hubs in a notes vault, and the names of work projects, and this command
/// writes into a public repository - so the one mistake it can make is the
/// one that cannot be taken back.
pub fn check_public(registry: &Registry) -> Result<()> {
    let json = serde_json::to_string(registry)?;
    // A drive letter or a UNC prefix is a path on someone's machine
    // whatever field it reached the file through.
    for mark in ["C:\\", "c:\\", "C:/", "\\\\", "/home/", "/Users/"] {
        if json.contains(mark) {
            bail!("the registry would publish `{mark}`, which is a path on a machine, not a fact about a product");
        }
    }
    for product in &registry.products {
        if let Some(repo) = &product.repository
            && !repo.starts_with("https://")
            && !repo.starts_with("git@")
        {
            bail!("{}: `{repo}` is not a public repository address", product.name);
        }
        if let Some(docs) = &product.docs
            && !docs.starts_with("https://")
        {
            bail!("{}: `{docs}` is not a public documentation address", product.name);
        }
    }
    Ok(())
}

/// Whether a colour is written the way the line writes colours.
pub fn check_accent(accent: &str) -> Result<()> {
    let ok = accent.len() == 7 && accent.starts_with('#') && accent[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        bail!("`{accent}` is not a colour; the line writes them as #RRGGBB");
    }
    Ok(())
}

/// Whether a mark's code is one of the line's.
pub fn check_code(code: &str) -> Result<()> {
    if code.chars().count() != 2 || !code.chars().all(|c| c.is_ascii_lowercase()) {
        bail!("`{code}` is not a mark's code; they are two lowercase letters");
    }
    Ok(())
}

/// Whether a form is one the line builds.
pub fn check_form(form: &str) -> Result<()> {
    if !FORMS.contains(&form) {
        bail!("`{form}` is not a form this line builds; they are {}", FORMS.join(", "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(name: &str) -> Product {
        Product {
            name: name.into(),
            about: Some("A sample product".into()),
            mark: Some("sa".into()),
            accent: Some("#3FA873".into()),
            accent2: None,
            form: Some("cli".into()),
            repository: Some("https://github.com/example/sample.git".into()),
            docs: Some("https://example.github.io/sample/".into()),
        }
    }

    fn registry(products: Vec<Product>) -> Registry {
        Registry {
            generator: "rigger",
            version: "0.0.0",
            products,
        }
    }

    #[test]
    fn a_public_registry_passes() {
        check_public(&registry(vec![product("sample")])).unwrap();
    }

    #[test]
    fn a_path_on_a_machine_is_refused_however_it_got_in() {
        // The check is on the rendered file rather than on the fields it
        // knows about, because the field that leaks will be the one added
        // after this test was written.
        let mut p = product("sample");
        p.about = Some("the one in C:\\Projects\\sample".into());
        let err = check_public(&registry(vec![p])).unwrap_err().to_string();
        assert!(err.contains("path on a machine"), "{err}");

        let mut p = product("sample");
        p.about = Some("kept under /home/someone/work".into());
        assert!(check_public(&registry(vec![p])).is_err());
    }

    #[test]
    fn a_repository_that_is_not_an_address_is_refused() {
        let mut p = product("sample");
        p.repository = Some("../sample".into());
        let err = check_public(&registry(vec![p])).unwrap_err().to_string();
        assert!(err.contains("public repository"), "{err}");
    }

    #[test]
    fn a_product_without_a_mark_is_still_published() {
        // A product recorded before its mark was drawn is part of the line;
        // leaving it out would make the registry disagree with the record
        // about what exists, which is the fault this command fixes.
        let mut p = product("sample");
        p.mark = None;
        p.accent = None;
        let json = serde_json::to_string(&registry(vec![p])).unwrap();
        assert!(json.contains("sample"), "{json}");
        // And the absent fields are absent, not null: a consumer reading
        // `accent` should get nothing rather than a colour named "null".
        assert!(!json.contains("accent"), "{json}");
    }

    #[test]
    fn the_shapes_of_a_mark_are_checked_rather_than_trusted() {
        check_code("rr").unwrap();
        assert!(check_code("RR").is_err(), "a code is lowercase");
        assert!(check_code("rrr").is_err(), "a code is two letters");

        check_accent("#8A62F0").unwrap();
        assert!(check_accent("8A62F0").is_err(), "a colour carries its hash");
        assert!(check_accent("#8A62F").is_err(), "six digits, not five");
        assert!(check_accent("#8A62FZ").is_err(), "hex digits only");

        check_form("cli").unwrap();
        let err = check_form("app").unwrap_err().to_string();
        assert!(err.contains("desktop"), "the error names what there is: {err}");
    }
}
