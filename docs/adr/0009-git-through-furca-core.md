# ADR 0009: Git is read through furca-core

- Status: accepted
- Date: 2026-10-07, amended 2026-11-07 (commits read by id)
- Amends: [0001](0001-rust.md) (the gix dependency), [0005](0005-facts-from-git.md) (how git is read)

## Context

`sync` and the activity of task cards read tags, commits and branches through gix directly. gix lives in 0.x majors, its API moves without a migration guide, and the knowledge of how to use it - which features to enable, how to peel an annotated tag, where a shallow clone ends - had to be relearnt in every product of the line that reads a repository. furca, the line's git client, publishes that knowledge as a library, `furca-core`, which reads in-process through gix and never spawns `git`.

## Decision

rigger reads git only through `furca-core` and has no gix dependency of its own. One module, `src/git.rs`, is the door: it opens a repository at exactly the recorded path (furca-core discovers upwards as git does; a directory inside another repository must not borrow its tags), and it reads a commit by its id.

A commit wanted by id - the commit a release tag points at, the tip of a card's branch - is read with furca-core's `Repository::commit`, a lookup rather than a walk. An id that names no commit (a tag on a tree, a commit a shallow clone does not hold) is left out; any other failure to read it is an error, so a damaged commit does not pass for a missing one.

## Consequences

- gix majors are absorbed once, in furca-core, for every product that reads git.
- `commits since the release` is what the newest tag cannot reach (`git log <tag>..HEAD`), so a branch begun before a release and merged after it counts as new work.
- Dating a tag costs the same however deep in the history its commit lies.
- A reference that cannot be read fails the listing of all references in furca-core; `sync` reports it as a warning instead of failing.
