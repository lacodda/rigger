---
title: version
description: One version whole, and aiming it at a week of the release calendar.
---

A version is a stage of a project's plan. `import` and `sync` record them; this command reads one whole, and gives one a place in the [calendar](/rigger/reference/calendar/).

## `version show`

```
rigger version show <PROJECT> [<VERSION>] [--json]
```

One version: its number and title, the week it is aimed at and that week's Friday, how far it got if it shipped, and its tasks with their ids. Without a version, the stage being built - the same one the [context packet](/rigger/reference/context/) calls current, so the two can never name different versions as "the next one".

```console
$ rigger version show sample
sample v0.3.0 · Search

aimed at   2026-W39 - releases on 2026-09-25
tasks      2 open of 3
  [4] done    full-text index
  [5] new     a query language
  [6] active  ranking
```

A shipped version says when, and how far past its tag it got - see [delivery](/rigger/reference/sync/#releases-and-publishing):

```console
$ rigger version show sample v0.2.0
sample v0.2.0 · Second

shipped    on 2026-09-11 - released and published
reached    crates.io, npm
tasks      0 open of 2
  [2] done    import
  [3] done    export
```

A version the record does not have is refused, like everywhere else a version is named; one written `v1.13` finds `v1.13.0`.

### The JSON is a contract

A release engine reads this to learn which number goes out on Friday and under what title - `furca release plan` is the first. So the shape is a contract: every field is named here, and the test suite fails when the JSON prints a field this page does not name.

```console
$ rigger version show sample --json
{
  "project": "sample",
  "version": "v0.3.0",
  "title": "Search",
  "status": "planned",
  "week": "2026-W39",
  "friday": "2026-09-25",
  "shipped_at": null,
  "delivery": null,
  "registries": [],
  "tasks": [
    { "id": 4, "title": "full-text index", "status": "done" },
    { "id": 5, "title": "a query language", "status": "new" }
  ],
  "open_tasks": 1
}
```

| Field | What it holds |
| --- | --- |
| `project` | the project, as the record names it |
| `version` | the version's number as the record spells it; `null` when every version has shipped and none is being built |
| `title` | the stage's title, or `null` |
| `status` | `planned` or `shipped` |
| `week` | the ISO week it is aimed at, as `2026-W39`, or `null` |
| `friday` | the Friday of that week, as `2026-09-25`, or `null` |
| `shipped_at` | the day it shipped, or `null` |
| `delivery` | how far past its tag it got: `tag-only`, `no-assets`, `released`, `publish-running`, `publish-failed`, `published`, or `null` when nobody has said |
| `registries` | the registries the release engine said it reached |
| `tasks` | its tasks in plan order, each with `id`, `title` and `status` |
| `open_tasks` | how many of those are still work to do |

Fields may be added; none is renamed or removed without a major version.

## `version plan`

```
rigger version plan <PROJECT> <VERSION> --week <WEEK>
rigger version plan <PROJECT> <VERSION> --clear
```

Aims a version at a week, or takes it off the calendar.

```console
$ rigger version plan sample v0.2.0 --week 2026-W37
v0.2.0 is aimed at 2026-W37 - the week of 2026-09-11
```

The week is ISO-8601 - `2026-W37`, and a typed `2026-W7` is understood too. The reply names the Friday, since that is the day the release is for.

Only a version the record already holds can be planned. A misspelt number is refused rather than created: a row nothing else knows about would sit in the calendar for ever, matching no plan, no changelog and no tag.

```console
$ rigger version plan sample v9.9.9 --week 2026-W37
error: no version 'v9.9.9' in the record; see `rigger project show`
```

Planning the same week twice says so and changes nothing. `--clear` removes the aim; the version stays in the plan, it simply has no week.

The week is a **plan**, never a claim about what happened. What happened comes from the tag, and where the two disagree the calendar shows the tag and names the distance. Nothing here overwrites a fact read from git ([ADR 0005](https://github.com/lacodda/rigger/blob/main/docs/adr/0005-facts-from-git.md)).

## Related

- [`calendar`](/rigger/reference/calendar/) - the grid this fills in.
- [`next`](/rigger/reference/next/) - one week of it, in full; its JSON is a contract too.
- [`sync`](/rigger/reference/sync/) - what reads the tag a plan is measured against.
