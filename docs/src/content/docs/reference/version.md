---
title: version
description: One version whole, and aiming it at a week of the release calendar.
---

A version is a stage of a project's plan. `version add` plans one, `import` reads them from a hub and `sync` from the tags; this command reads one whole, and gives one a place in the [calendar](/rigger/reference/calendar/).

## `version add`

```
rigger version add <PROJECT> <VERSION> [--title <TEXT>] [--task <TEXT>]... [--week <WEEK>] [--json]
```

Plans a stage: its number, what it is about, its tasks and, if you know it, the week it is aimed at. No hub is needed - this is how a project with no notes of its own gets a plan.

```console
$ rigger version add sample v0.1.0 --title "First light" --task "read the sheet" --task "write the report" --week 2026-W43
Added v0.1.0 to sample, with 2 tasks
  aimed at 2026-W43 - the week of 2026-10-23
```

Asked again, it adds what is new and nothing else. A stage is found by value - `v0.1` is `v0.1.0` - and a task by its text, so naming tasks it already has adds none, and a new one goes last in its list. A title given replaces the stage's title; one left out keeps it.

```console
$ rigger version add sample v0.1 --task "read the sheet" --task "draw the chart"
v0.1.0 was in the plan already; 1 task added
```

A stage that shipped takes no new tasks - a task for it is a task for the next one - and text that is not a version (`next`, `0.3`) is refused: a version is written the way a tag would be. A new stage goes last in the plan, and an [export](/rigger/reference/export/) to a hub writes it there; read back, the hub changes nothing. Under `--json` it answers with the version as `version show` prints it.

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

`version plan --json` answers with the same document for the version it aimed.

<!-- json: version show, version plan, version add -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The project, as the record names it. |
| `version` | string | The version's number as the record spells it. |
| `title` | string or null | The stage's title, or `null`. |
| `status` | string | `planned` or `shipped`. |
| `week` | string or null | The ISO week it is aimed at, as `2026-W39`, or `null`. |
| `friday` | string or null | The Friday of that week, as `2026-09-25`, or `null`. |
| `shipped_at` | string or null | The day it shipped, as `YYYY-MM-DD`, or `null`. |
| `delivery` | string or null | How far past its tag it got: `tag-only`, `no-assets`, `released`, `publish-running`, `publish-failed` or `published`; `null` when nobody has said. |
| `registries` | array | The registries the release engine said it reached. |
| `registries[]` | string | One registry's name. |
| `tasks` | array | The version's tasks, in plan order. |
| `tasks[]` | object | One task. |
| `tasks[].id` | integer | The task's id. |
| `tasks[].title` | string | The task's title. |
| `tasks[].status` | string | The task's status, such as `new`, `active` or `done`. |
| `tasks[].snoozed_until` | string | The day the task sleeps until, as `YYYY-MM-DD`; present only while it is asleep. |
| `open_tasks` | integer | How many of the tasks are still work to do. |

When every version has shipped and none is being built, `version show` prints only `project` and a `version` of `null`.

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
