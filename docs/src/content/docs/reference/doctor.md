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
schema:    version 4
projects:  1
versions:  0
tasks:     0
sessions:  0
events:    0
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
