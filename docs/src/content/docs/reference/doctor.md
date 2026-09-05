---
title: doctor
description: Where the database is and what it holds.
---

```
rigger doctor [--json]
```

Prints the database path, its schema version and how many records of each kind it holds. Before `init` it says so instead of failing, so `doctor` is always safe to run first.

```console
$ rigger doctor
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
schema:    version 4
projects:  1
versions:  0
tasks:     0
sessions:  0
events:    0
```

With `--json`:

```json
{
  "database": "C:\\Users\\you\\AppData\\Local\\lacodda\\rigger\\data\\rigger.db",
  "initialised": true,
  "schema_version": 4,
  "counts": { "projects": 1, "versions": 0, "tasks": 0, "sessions": 0, "events": 0 }
}
```

## Where the plan and git disagree

Once [`rigger sync`](/rigger/reference/sync/) has read a project, `doctor` lists the versions its plan closed that no tag confirms - across every project, without reading git again:

```console
$ rigger doctor
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
schema:    version 4
projects:  1
versions:  3
tasks:     1
sessions:  0
events:    0

closed in the plan, no tag in git (1):
  claimed      v0.2.0
  a tag would settle it; rigger does not change what you wrote
```

Reported, never corrected. A missing tag is not proof a release did not happen - it may simply never have been fetched - and silently reopening a version would erase what you wrote ([ADR 0005](https://github.com/lacodda/rigger/blob/main/docs/adr/0005-facts-from-git.md)).

A project that has never been synced is named rather than judged: without a sync there is no way to tell a claim from a fact, and `doctor` says which projects it cannot speak for.

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

## What it leaves out

A [service project](/rigger/reference/project/) never appears among the projects waiting to be synced. There is no repository to read, so it would have sat in that list for ever being advised a command that could not help it.

## Related

- [`init`](/rigger/reference/init/) - create the database `doctor` reports on.
- [`sync`](/rigger/reference/sync/) - what fills the mismatch section.
