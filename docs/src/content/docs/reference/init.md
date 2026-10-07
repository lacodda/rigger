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
Created C:\Users\you\AppData\Local\lacodda\rigger\data\config.toml with the 'line' profile
Created C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db (schema version 27)
Next: rigger project add <path>

$ rigger init
Already initialised: C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db
```

Every other command opens this database and, when it was written by an older rigger, migrates it forward on the spot. A database written by a newer rigger is refused with a message to update.

With `--json` it prints what was done.

<!-- json: init -->
| Field | Type | Meaning |
| --- | --- | --- |
| `config` | string | The path of `config.toml`. |
| `config_created` | boolean | Whether this run wrote the config. |
| `profile` | string | The current profile's name. |
| `database` | string | The database path. |
| `created` | boolean | Whether this run created the database. |
| `schema` | integer | The schema version the database holds. |

## Where the database lives

| OS | Directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\lacodda\rigger\data` |
| macOS, Linux | the platform's local data directory for `rigger` |

`RIGGER_DATA_DIR` overrides the directory for every command; [`rigger doctor`](/rigger/reference/doctor/) prints the exact path in use.

## Related

- [`project`](/rigger/reference/project/) - record the first repository.
- [`doctor`](/rigger/reference/doctor/) - where the database is and what it holds.
