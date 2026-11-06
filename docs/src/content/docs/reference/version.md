---
title: version
description: Plan a version, and read one whole.
---

A version is a stage of a project's plan. `version add` plans one, `import` reads them from a hub and `sync` from the tags; this command plans one and reads one whole.

## `version add`

```
rigger version add <PROJECT> <VERSION> [--title <TEXT>] [--task <TEXT>]... [--json]
```

Plans a stage: its number, what it is about, and its tasks. No hub is needed - this is how a project with no notes of its own gets a plan.

```console
$ rigger version add sample v0.1.0 --title "First light" --task "read the sheet" --task "write the report"
Added v0.1.0 to sample, with 2 tasks
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

One version: its number and title, how far it got if it shipped, and its tasks with their ids. Without a version, the stage being built - the same one the [context packet](/rigger/reference/context/) calls current, so the two can never name different versions as "the next one".

```console
$ rigger version show sample
sample v0.3.0 · Search

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

A release engine reads this to learn which number goes out next and under what title - `furca release plan` is the first. So the shape is a contract: every field is named here, and the test suite fails when the JSON prints a field this page does not name.

```console
$ rigger version show sample --json
{
  "project": "sample",
  "version": "v0.3.0",
  "title": "Search",
  "status": "planned",
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

<!-- json: version show, version add -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The project, as the record names it. |
| `version` | string | The version's number as the record spells it. |
| `title` | string or null | The stage's title, or `null`. |
| `status` | string | `planned` or `shipped`. |
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

## Related

- [`queue`](/rigger/reference/queue/) - the stage that is current is the one the queue names.
- [`sync`](/rigger/reference/sync/) - what reads the tag a plan is measured against.
