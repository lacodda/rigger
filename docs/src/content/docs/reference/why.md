---
title: why
description: The decisions, findings and changes that led to a version.
---

```
rigger why <PROJECT> <VERSION> [--json]
```

Shows the work that went into one release: what was decided, what was learnt, what was tripped over, and what changed - everything recorded between the release before it and itself.

```console
$ rigger why sample v0.3.0
v0.3.0 — shipped 2026-09-04
the work after v0.2.0 (2026-09-04)

2026-09-04  decision  The budget is a gate, not a suggestion.
2026-09-04  pitfall   A wide window hides what the budget dropped.
2026-09-04  change    feat: rank what a person wrote above a commit
```

No event carries a version of its own, and none needs to: a release is bounded by the release before it, so the record can answer this for versions that shipped long before the question was asked.

## How the window is drawn

From the previous release to this one, by the moment each was tagged rather than the day. A day would not be enough - this line ships several versions in an afternoon, and a day-wide window would hand every one of them the whole day's work.

The lower bound is exclusive and the upper inclusive. That asymmetry is the point: a tag points *at* a commit, so a release and its last commit share a moment. Inclusive at the top keeps that commit in the release it shipped; exclusive at the bottom keeps it out of the next one.

Two releases tagged in the same second have nothing between them, and `why` says so rather than showing an empty answer that looks like a missing record.

A version imported from a hub carries only the day it shipped, which still bounds the window - less precisely, but better than not at all. Run [`rigger sync`](/rigger/reference/sync/) and the tag supplies the moment.

## A version still being built

The current stage has no upper bound, so its window runs to now:

```console
$ rigger why sample v0.4.0
v0.4.0 · Search — being built
the work after v0.3.0 (2026-09-04)

2026-09-04  decision  A bare word is searched as a prefix.
```

That is the work in progress, which is exactly what a session picking up the stage wants to see.

## Spelling the version

The number is matched by value, not by text, so `v1.9`, `1.9` and `v1.9.0` all find the stage recorded as `v1.9.0` - hubs spell a version several ways and nobody remembers which.

A version the record does not have is refused, with a pointer to what it does have.

## Related

- [`find`](/rigger/reference/find/) - searching by word rather than by version.
- [`sync`](/rigger/reference/sync/) - what supplies the tags this window is drawn from.
- [`context`](/rigger/reference/context/) - the current stage and its open tasks.
