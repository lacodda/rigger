---
title: project
description: Add, list and show projects.
---

A project is a repository rigger keeps a record of. This release records the repository itself; versions, tasks and events attach to it in the releases that follow.

## `project add`

```
rigger project add <PATH> [--name <NAME>]
```

Records the repository at `PATH`. The name is the directory name, because that is what a project is called in conversation - a manifest often declares something else, such as the crate `sample-cli` for the product `sample`, or a `*-workspace` root. Pass `--name` when the directory is not it either. The `origin` remote is read from the repository's git config when there is one.

```console
$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git
```

A path recorded before, or a name already taken, is refused with the existing project named in the message.

## `project list`

```
rigger project list [--json]
```

```console
$ rigger project list
sample  C:\dev\sample
```

With `--json`, an array of the objects `project show --json` prints.

## `project show`

```
rigger project show <NAME> [--json]
```

```console
$ rigger project show sample
sample
  path:    C:\dev\sample
  remote:  https://github.com/acme/sample.git
  since:   2026-09-04T07:41:10Z

$ rigger project show sample --json
{
  "id": 1,
  "name": "sample",
  "path": "C:\\dev\\sample",
  "remote": "https://github.com/acme/sample.git",
  "created_at": "2026-09-04T07:41:10Z",
  "tier": "A",
  "rhythm_weeks": 2
}
```

Timestamps are UTC in RFC 3339. `tier` and `rhythm_weeks` are null until `project tier` sets them.

## `project tier`

```
rigger project tier <NAME> <A|B|C|out> [--rhythm <WEEKS>]
```

Where a project sits in the release rotation, and how often it should ship. The tiers are the ones the line already works to: **A** carrying products that are released and used every day, **B** growing ones whose code works but whose circuit is not closed, **C** declared ones with a name and a plan and no product yet.

```console
$ rigger project tier sample A
sample is tier A - carrying - released, in the registries, used every day
  a release every 2 weeks
```

Each tier carries a rhythm of its own - two weeks for A, four for B, six for C - so setting one is a single word in the common case. `--rhythm` is for the project that keeps its tier's company but not its pace.

`out` is for a project deliberately outside the rotation: it is worked on when asked, and [`next`](/rigger/reference/next/) never says it is behind. That is a decision, not an omission - a project given `out` **and** a rhythm is still left alone.

A project with no tier at all is out by omission, and is equally left alone: rigger does not invent a schedule that nobody asked for.

## `rigger project service <name>`

Records a place the record keeps for itself: a project with no repository, and none coming.

```console
$ rigger project service line
Recorded 'line' as a place the record keeps for itself
  no repository: sync will not ask git about it
```

There is one thing this exists for. A [`retro`](/rigger/reference/retro/) is about every project at once, so its summary belongs to none of them - filing it under one would make it findable from the wrong place and invisible from the rest.

Such a place is not a repository that happens to be missing, and saying so matters: recorded as an ordinary project it made [`sync`](/rigger/reference/sync/) warn on every run that there was no git to read, and [`doctor`](/rigger/reference/doctor/) list it for ever among the projects waiting to be synced - the record nagging about a project working exactly as intended. Now `sync` does not ask git about it, and `doctor` does not count it as unsynced.

Everything else works on it as usual: events, questions, wishes, [`digest`](/rigger/reference/digest/), [`find`](/rigger/reference/find/) and the [context packet](/rigger/reference/context/).

## Related

- [`init`](/rigger/reference/init/) - the database `project` writes to.
- [`doctor`](/rigger/reference/doctor/) - how many projects are recorded.
- [`calendar`](/rigger/reference/calendar/) - where a tier and its rhythm are read.
- [`retro`](/rigger/reference/retro/) - what a service project is made for.
