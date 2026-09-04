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
schema:    version 3
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
  "schema_version": 3,
  "counts": { "projects": 1, "versions": 0, "tasks": 0, "sessions": 0, "events": 0 }
}
```

## Where the plan and git disagree

Once [`rigger sync`](/rigger/reference/sync/) has read a project, `doctor` lists the versions its plan closed that no tag confirms - across every project, without reading git again:

```console
$ rigger doctor
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
schema:    version 3
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

Later releases add another section here: hubs edited by hand after export.

## Related

- [`init`](/rigger/reference/init/) - create the database `doctor` reports on.
- [`sync`](/rigger/reference/sync/) - what fills the mismatch section.
