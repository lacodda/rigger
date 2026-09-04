---
title: backup
description: Copy the database aside.
---

```
rigger backup
```

Copies the database beside itself, stamped with the schema it holds and the moment it was taken.

```console
$ rigger backup
Copied to C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.v1-20260904-084102.bak
```

The copy goes through SQLite's own backup interface rather than a file copy, so it is consistent even if something else has the database open.

## Before a migration, automatically

The record is the truth here, and a migration rewrites it. So when a newer rigger opens a database written by an older one, it takes this same copy first and says where it went:

```console
$ rigger doctor
Migrating schema 1 -> 2; the previous database is saved as ...\rigger.v1-20260904-084102.bak
```

A fresh database has nothing to lose and is not copied.

## Restoring

A backup is an ordinary SQLite database. To go back to one, close anything using rigger and copy the file over `rigger.db` - the path [`doctor`](/rigger/reference/doctor/) prints. Backups are never removed by rigger; delete the ones you no longer want.

## Related

- [`doctor`](/rigger/reference/doctor/) - where the database is.
- [`import`](/rigger/reference/import/) - the command most worth a backup first.
