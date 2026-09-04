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
schema:    version 1
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
  "schema_version": 1,
  "counts": { "projects": 1, "versions": 0, "tasks": 0, "sessions": 0, "events": 0 }
}
```

Later releases add sections here: mismatches between the plan and git, hubs edited by hand after export.

## Related

- [`init`](/rigger/reference/init/) - create the database `doctor` reports on.
