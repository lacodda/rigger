# ADR 0011: A repository's release.toml outranks the record

- Status: accepted
- Date: 2026-11-06
- Extends: [0005](0005-facts-from-git.md) (a fact in the repository outranks a claim in the record)

## Context

The gate of a project lived in two places: the record's `gate` field, and `[gate] commands` in the `release.toml` the release engine reads. The file is public and has to work in CI with no record around, so it cannot be a copy of the field. The field could only drift from it, and the engine had to warn when it did.

The record also had no way to tell "released, not installed" from "released, and this project is never installed": a library has no install to miss.

## Decision

`release.toml` is the truth for what it states. `src/releasefile.rs` reads two things from it, and nothing else: the gate, and whether a release ends with an install or a deploy (`[post.install]`, `[post.deploy]`). The rest of the file is the engine's.

- The gate in force is the file's when it names commands, and the record's field otherwise. Every reader takes that one, and `project show --json` prints it, so the engine's comparison has nothing left to disagree about.
- `project set --gate` is refused where the file states a gate, as `--about` is refused where a manifest describes the project.
- A file that cannot be read is not treated as absent: `rigger gate` refuses, and a screen goes on with no gate and says why on standard error.
- Whether a version was installed or deployed is what the engine says it did, in `installed_at` and `deployed_at`. The file decides only whether the absence of that word means anything: a version owes an install only if the project's release ends with one.

## Consequences

- A gate has one place to change for any project with a `release.toml`. The record's field remains for projects without one, and stays unused rather than being cleared when a file appears, so removing the file does not lose it.
- A read of the record can touch the working tree: the packet and the screens read one small file per project. That is a read of what is checked out, which can differ from the last release's commit; it is the same file the next release will read.
- What a release owes is said only for the newest shipped version. An older one is history, and a new promise in the file does not reach back to it.
