# ADR 0009: Git is read through furca-core

- Status: accepted
- Date: 2026-10-07
- Amends: [0001](0001-rust.md) (the gix dependency), [0005](0005-facts-from-git.md) (how git is read)

## Context

`sync` and the activity of task cards read tags, commits and branches through gix directly. gix lives in 0.x majors, its API moves without a migration guide, and the knowledge of how to use it - which features to enable, how to peel an annotated tag, where a shallow clone ends - had to be relearnt in every product of the line that reads a repository. furca, the line's git client, publishes that knowledge as a library, `furca-core`, which reads in-process through gix and never spawns `git`.

## Decision

rigger reads git only through `furca-core` and has no gix dependency of its own. One module, `src/git.rs`, is the door: it opens a repository at exactly the recorded path (furca-core discovers upwards as git does; a directory inside another repository must not borrow its tags), and it finds commits by id.

furca-core 0.3 reads a commit only on the way through a walk. Until a release of it reads one by revision, a commit wanted by id - the commit a release tag points at, the tip of a card's branch - is looked for in a walk over every branch and tag: the newest 512 commits first, widening eightfold until every id is found or the history ends.

## Consequences

- gix majors are absorbed once, in furca-core, for every product that reads git.
- `commits since the release` is what the newest tag cannot reach (`git log <tag>..HEAD`), so a branch begun before a release and merged after it counts as new work.
- Dating an old tag costs a walk to it. On the line's repositories that is milliseconds; on a repository of a hundred thousand commits it is one full walk per `sync`, which ends when furca-core's `Repository::commit` is released and the walk is replaced by a lookup.
- A reference that cannot be read fails the listing of all references in furca-core; `sync` reports it as a warning instead of failing.
