# ADR 0008: Registry publishing via OIDC trusted publishing

- Status: accepted
- Date: 2026-09-04

## Context

Releases are cut by a tag and must reach GitHub Releases, crates.io and npm without anyone holding a long-lived registry token in repository secrets. Both registries support OIDC trusted publishing from GitHub Actions. Neither can bind trust to a package that does not exist yet, so the very first publication has to be done by hand.

## Decision

`release.yml` builds the target matrix and creates the GitHub Release with notes from git-cliff. `publish.yml` publishes to crates.io and npm with OIDC tokens exchanged at run time. The first publication to each registry is manual; after the names are claimed and trust is configured, `publish.yml` gains the tag trigger and the flow is fully automatic. The npm page shows the root README, copied in at publish time rather than kept as a second file.

## Consequences

- No registry token is ever stored in the repository's secrets.
- `tests/release_consistency.rs` holds the facts the three pages must agree on - versions, description, one README, absolute links, real commands - so a mismatch fails CI instead of shipping.
- The tag trigger on `publish.yml` is a deliberate second step, recorded in the changelog of the release that enables it.
