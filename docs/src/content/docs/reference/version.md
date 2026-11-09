---
title: version
description: Plan a version, and read one whole.
---

A version is a stage of a project's plan. `version add` plans one, `import` reads them from a hub and `sync` from the tags; this command plans one, renumbers one and reads one whole.

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

A stage that shipped takes no new tasks - a task for it is a task for the next one - and a number the plan struck is free to plan again. Text that is not a version (`next`, `0.3`) is refused: a version is written the way a tag would be. A new stage goes last in the plan, and an [export](/rigger/reference/export/) to a hub writes it there; read back, the hub changes nothing. Under `--json` it answers with the version as `version show` prints it.

## `version move`

```
rigger version move <PROJECT> <FROM> <TO> [--shift] [--json]
```

Gives a stage still to come another number, in the record. It is the same stage afterwards: its tasks, their history and the links anchored on it follow without anything being matched, and its heading says the new number.

```console
$ rigger version move sample v0.5.0 v0.7.0
v0.5.0 is now v0.7.0
```

A number another stage holds is refused. `--shift` opens a place instead: the stage standing there moves up one, and the one behind it if it is in the way, as far as the first free number and no further. That is how a stage is inserted into the middle of a plan.

```console
$ rigger version move sample v0.7.0 v0.3.0 --shift
v0.7.0 is now v0.3.0
v0.3.0 is now v0.4.0
v0.4.0 is now v0.5.0
```

A stage crossing a heading of the plan changes block with its number, and the prose between the blocks stays where it stands. A shift keeps how the plan spells its numbers: `v1.9` is followed by `v1.10`, `v1.10.1` by `v1.10.2`.

What it will not do:

- Move a stage that shipped: its number belongs to its tag.
- Put a stage at or behind the last release: a stage still to come goes after it.
- Shift a stage that opens a major, like `v1.0.0`, which has no next number; move it by name if that is what is meant.

A number the plan struck is free. The struck stage keeps its history under the spelling `v0.4.0+struck-12`, which reads as the same number and is never preferred over a stage standing there.

Under `--json` it answers with the plan as `version list` prints it, and what moved. Moving a stage to the number it has moves nothing.

<!-- json: version move -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The project. |
| `moved` | array | What changed number, the stage asked for first. |
| `moved[]` | object | One stage. |
| `moved[].from` | string | The number it had. |
| `moved[].to` | string | The number it has now. |
| `versions` | array | The plan as it now stands, as `version list` prints it. |
| `versions[]` | object | One version. |
| `versions[].version` | string | The version. |
| `versions[].title` | string or null | What the stage is about. |
| `versions[].status` | string | `planned`. |
| `versions[].shipped_at` | string or null | The day it shipped; `null` for a stage still to come. |
| `versions[].tasks` | integer | Its tasks, the dropped ones aside. |
| `versions[].open` | integer | How many of them are open. |
| `versions[].current` | boolean | It is the stage the project is on. |
| `skipped` | array | Numbers skipped between one version and the next, as `v0.4`. |
| `skipped[]` | string | One of them. |

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
installed  on 2026-09-11
deployed   not yet
tasks      0 open of 2
  [2] done    import
  [3] done    export
```

`installed` and `deployed` show what the [release engine said](/rigger/reference/note/#installed-deployed); `not yet` is what the newest version still owes by its project's `release.toml`, and a step the file does not promise is not shown.

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
  "installed_at": null,
  "deployed_at": null,
  "owed": [],
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
| `installed_at` | string or null | When the release engine first said it was installed for the owner, in RFC 3339 UTC. |
| `deployed_at` | string or null | When the release engine first said it was deployed, in RFC 3339 UTC. |
| `owed` | array | What the newest shipped version's `release.toml` promises for after the publish and nobody has said was done; empty for any other version. |
| `owed[]` | string | `install` or `deploy`. |
| `tasks` | array | The version's tasks, in plan order. |
| `tasks[]` | object | One task. |
| `tasks[].id` | integer | The task's id. |
| `tasks[].title` | string | The task's title. |
| `tasks[].status` | string | The task's status, such as `new`, `active` or `done`. |
| `tasks[].snoozed_until` | string | The day the task sleeps until, as `YYYY-MM-DD`; present only while it is asleep. |
| `tasks[].woke` | string or null | The day the task came back from sleep as `YYYY-MM-DD`, while that is today or yesterday; `null` otherwise. |
| `open_tasks` | integer | How many of the tasks are still work to do. |

When every version has shipped and none is being built, `version show` prints only `project` and a `version` of `null`.

Fields may be added; none is renamed or removed without a major version.

## `version list`

```console
$ rigger version list sample
> v0.2.0  2 open of 3       Plugins
  v0.3.0  1 open of 1       Themes
  v0.5.0  4 open of 4       Sync

Skipped in the numbering: v0.4
```

The whole plan of a project, in order, with the stage it is on marked. A plan's number is the theme's, not the release's, so a gap here is not an error, but it is worth a look: it may be a version that was never written down. `--all` adds what has shipped.

<!-- json: version list -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The project. |
| `versions` | array | Its versions in order; with `--all`, the shipped ones too. |
| `versions[]` | object | One version. |
| `versions[].version` | string | The version. |
| `versions[].title` | string or null | What the stage is about. |
| `versions[].status` | string | `planned` or `shipped`. |
| `versions[].shipped_at` | string or null | The day it shipped. |
| `versions[].tasks` | integer | Its tasks, the dropped ones aside. |
| `versions[].open` | integer | How many of them are open. |
| `versions[].current` | boolean | It is the stage the project is on. |
| `skipped` | array | Numbers skipped between one version and the next, as `v0.4`. |
| `skipped[]` | string | One of them. |

## Related

- [`queue`](/rigger/reference/queue/) - the stage that is current is the one the queue names.
- [`sync`](/rigger/reference/sync/) - what reads the tag a plan is measured against.
