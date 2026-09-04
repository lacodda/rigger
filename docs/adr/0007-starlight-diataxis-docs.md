# ADR 0007: Docs: Astro Starlight structured by Diátaxis

- Status: accepted
- Date: 2026-09-04

## Context

The README is a "day in the life", not a reference. Every command still needs a page, concepts need explaining once rather than on each page, and the site must build and deploy from CI without committing artefacts. Sibling products of the line already use one stack, and a reader moving between them should not relearn the layout.

## Decision

Documentation lives in `docs/` as an Astro Starlight site with its own `package.json`, structured by Diátaxis: Getting Started, Guides, Concepts, Reference. Reference has one page per top-level command, titled with the bare command name; what is common to all commands is stated once on the overview page. Links inside the site are absolute with the base path. The site builds and deploys to GitHub Pages only in CI.

## Consequences

- A change to the command surface and its docs page are one commit; a command without a page does not exist.
- The docs package is isolated from the crate: `cargo publish` excludes `docs/`.
- The sidebar is generated from page titles, so titles carry no product prefix.
