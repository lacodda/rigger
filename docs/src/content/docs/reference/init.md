---
title: init
description: Create the database.
---

```
rigger init
```

Creates the data directory and the database inside it, migrated to the current schema. Running it again is harmless: an existing database is reported and left as it is.

```console
$ rigger init
Created C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db (schema version 1)
Next: rigger project add <path>

$ rigger init
Already initialised: C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
```

Every other command opens this database and, when it was written by an older rigger, migrates it forward on the spot. A database written by a newer rigger is refused with a message to update.

## Where the database lives

| OS | Directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\lacodda\rigger\data` |
| macOS, Linux | the platform's local data directory for `rigger` |

`RIGGER_DATA_DIR` overrides the directory for every command; [`rigger doctor`](/rigger/reference/doctor/) prints the exact path in use.

## Related

- [`project`](/rigger/reference/project/) - record the first repository.
- [`doctor`](/rigger/reference/doctor/) - where the database is and what it holds.
