---
title: doctor
description: Where the database is and what it holds.
---

```
rigger doctor [--json]
```

Prints the [profile](/rigger/reference/profile/) in use, the database path, its schema version and how many records of each kind it holds. Before `init` it says so instead of failing, so `doctor` is always safe to run first.

```console
$ rigger doctor
profile:   line (C:\Users\you\AppData\Local\lacodda\rigger\data\config.toml)
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db
schema:    version 27
projects:  1
versions:  0
tasks:     0
sessions:  0
events:    0
mcp:       answers - protocol 2025-06-18, 24 tools, 1 prompt
backup:    today, 3 copies kept
```

With `--json`:

```json
{
  "database": "C:\\Users\\you\\AppData\\Local\\lacodda\\rigger\\data\\rigger.db",
  "initialised": true,
  "schema_version": 4,
  "counts": { "projects": 1, "versions": 0, "tasks": 0, "sessions": 0, "events": 0 },
  "backup": { "newest_at": "2026-09-15T12:02:53Z", "age_days": 0, "copies": 3, "state": "fresh" }
}
```

Before `init`, only `profile`, `database` and `initialised` (false) are printed.

<!-- json: doctor -->
| Field | Type | Meaning |
| --- | --- | --- |
| `profile` | string | The name of the profile in use. |
| `database` | string | The database path. |
| `initialised` | boolean | Whether the database exists. |
| `schema_version` | integer | The schema version the database holds. |
| `counts` | object | How many records of each kind the database holds. |
| `counts.projects` | integer | Projects. |
| `counts.versions` | integer | Versions. |
| `counts.tasks` | integer | Tasks. |
| `counts.sessions` | integer | Sessions. |
| `counts.events` | integer | Events. |
| `mcp` | object | What the built-in MCP server answered to a self-check. |
| `mcp.version` | string | The server's version. |
| `mcp.protocol` | string | The MCP protocol version it speaks. |
| `mcp.tools` | integer | How many tools it offers. |
| `mcp.prompts` | integer | How many prompts it offers. |
| `backup` | object | How old the newest database copy is. |
| `backup.newest_at` | string or null | When the newest copy was taken, in RFC 3339 UTC; null when there is none. |
| `backup.age_days` | integer or null | Whole days since then; null when there is no copy. |
| `backup.copies` | integer | How many copies are kept. |
| `backup.state` | string | `fresh` (under a day), `stale` (a day or more), `old` (a week or more) or `none`. |
| `hubs` | array or null | Hub files the record cannot vouch for; null unless `--hubs` is given. |
| `hubs[]` | object | One such hub file. |
| `hubs[].project` | string | The project's name. |
| `hubs[].file` | string | The file, several joined by `, ` when kept by hand, or `-` when no hub is known. |
| `hubs[].why` | string | The reason the file cannot be vouched for. |
| `closed_without_a_tag` | array | Versions the plan closed that no tag confirms. |
| `closed_without_a_tag[]` | object | One such version. |
| `closed_without_a_tag[].project` | string | The project's name. |
| `closed_without_a_tag[].version` | string | The version. |
| `one_step_apart` | array | Places where the plan and the tags look one step apart. |
| `one_step_apart[]` | object | One such place: two neighbouring versions of one project. |
| `one_step_apart[].project` | string | The project's name. |
| `one_step_apart[].closed` | string | The version the plan closed with no tag to show for it. |
| `one_step_apart[].tagged` | string | Its neighbour: a tag the plan never named a stage for. |
| `never_synced` | array | Projects `sync` has never read. |
| `never_synced[]` | string | A project's name. |
| `wishes_left_in_the_hub` | array | Projects with wishes waiting in `Хотелки.md`. |
| `wishes_left_in_the_hub[]` | object | One such project. |
| `wishes_left_in_the_hub[].project` | string | The project's name. |
| `wishes_left_in_the_hub[].wishes` | integer | How many wishes wait. |

## How old the insurance is

The record is one file. `backup:` says when the newest copy of it was taken, and how many are kept:

```console
$ rigger doctor
...
backup:    3 days ago, 5 copies kept - older than a day; run `rigger backup`
```

A copy from today passes without advice, older than a day is worth a line, and older than a week (`"state": "old"`) means the copy no longer resembles the record. Having none at all says so plainly:

```console
backup:    none - one file, no copy of it; run `rigger backup`
```

The age is read from the moment in the copy's name, not from the file's own timestamp, which says when it was last moved rather than when its contents were true. See [`backup`](/rigger/reference/backup/).

## Where the plan and git disagree

Once [`rigger sync`](/rigger/reference/sync/) has read a project, `doctor` lists the versions its plan closed that no tag confirms - across every project, without reading git again:

```console
$ rigger doctor
profile:   line (C:\Users\you\AppData\Local\lacodda\rigger\data\config.toml)
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db
schema:    version 27
projects:  1
versions:  3
tasks:     1
sessions:  0
events:    1
mcp:       answers - protocol 2025-06-18, 24 tools, 1 prompt
backup:    today, 1 copy kept

closed in the plan, no tag in git (1):
  claimed      v0.2.0
  a tag would settle it; rigger does not change what you wrote
```

Reported, never corrected. A missing tag is not proof a release did not happen - it may simply never have been fetched - and silently reopening a version would erase what you wrote ([ADR 0005](https://github.com/lacodda/rigger/blob/main/docs/adr/0005-facts-from-git.md)).

### One step apart

Either half alone is ordinary - a tag not fetched yet, a release older than the plan. Side by side they are the shape of a plan running a step ahead of its tags: a version closed without a tag, right beside a tag the plan never named a stage for. Since v0.27.0 `doctor` names each such pair:

```console
$ rigger doctor
...
the plan and the tags look one step apart (1):
  sample       v0.2.0 closed without a tag, beside v0.3.0 tagged without a stage
  the tags may name the stage before the one the plan gave them; renumber the plan, or tag the stage
```

Reported, like the list above it, and never corrected: which of the two is right is a question about the work, and the record cannot answer it. Tagging the closed stage, or renumbering the plan so its stages carry the numbers the tags gave them, settles it. Under `--json` the pairs are `one_step_apart`.

A project that has never been synced is named rather than judged: without a sync there is no way to tell a claim from a fact, and `doctor` says which projects it cannot speak for.

## Wishes left in the hub

`Хотелки.md` stopped being read at every session in v0.20.0: a wish now arrives through [`rigger wish`](/rigger/reference/note/) or the assistant's tool and lives in the record. Which means a wish written into the file after that is a wish nobody would ever read - so `doctor` says when one is waiting:

```console
wishes left in Хотелки.md (1):
  atlas        1 wish
  the file is no longer read at every session: `rigger import <project> --hub <dir>` takes them in
```

Checked without being asked: it is one small file per project that has a hub. The template, its placeholder and a note saying the wishes were already sorted do not count.

## Hubs (`--hubs`)

A generated file that somebody has edited has stopped being a view of the record, and the next [`export`](/rigger/reference/export/) would overwrite the edit without saying so. `--hubs` names those files. It is off by default because it reads every hub from disk.

```console
$ rigger doctor --hubs

hubs the record cannot vouch for (1):
  sample       План.md        edited since it was generated
  edited: `rigger import` takes the edit into the record; `rigger export` discards it
```

Both ways out are named, because either can be the right one: the edit was worth making, or it was not.

A project whose hub the record has never seen is named too, rather than passed over:

```console
  gamma        -              no hub recorded; import or export one
```

That is not pedantry. A check that skips what it cannot find prints the same clean line as a check that looked and found nothing wrong - and the record learns where a hub is only when one is imported or exported, so before that it genuinely cannot speak for it.

A hub still kept by hand is named as such, with the files that are:

```console
  beta         План.md, Изменения.md  kept by hand; `rigger export --adopt` hands it over
```

It is the one thing "every project through rigger" has left to do, so the list says how far along a line is rather than staying quiet about the files it skipped.

## What it leaves out

A [service project](/rigger/reference/project/) never appears among the projects waiting to be synced. There is no repository to read, so it would have sat in that list for ever being advised a command that could not help it.

## Related

- [`init`](/rigger/reference/init/) - create the database `doctor` reports on.
- [`sync`](/rigger/reference/sync/) - what fills the mismatch section.
