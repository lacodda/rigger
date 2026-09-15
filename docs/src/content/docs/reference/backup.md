---
title: backup
description: Copy the database aside, and keep the last few.
---

```
rigger backup [--keep N] [--list]
```

Copies the database beside itself, stamped with the schema it holds and the moment it was taken, and deletes all but the newest few.

```console
$ rigger backup
Copied to C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.v18-20260915-120253.bak
Kept the 10 newest, deleted 2 older copies.
```

The copy goes through SQLite's own backup interface rather than a file copy, so it is consistent even if something else has the database open.

## Keeping a few, not all

`--keep N` sets how many copies survive; the default is 10. Rotation never deletes the copy it has just taken, so `--keep 0` still leaves one.

```console
$ rigger backup --keep 2
Copied to ...\rigger.v18-20260915-120253.bak
Kept the 2 newest, deleted 2 older copies.
```

`--list` shows what is there, newest first, without taking a copy:

```console
$ rigger backup --list
2 copies newest first:
  rigger.v18-20260915-120253.bak           today
  rigger.v18-20260915-120252.bak           today
```

The age comes from the moment in the name, not from the file's own timestamp: a copy that has been moved or restored carries a timestamp saying when it was moved, while the name still says when the record inside it was true. A file whose name holds no readable moment is listed as `unknown` rather than given a guessed age.

## Automatically, at the end of a session

The record is one file, and a sitting that has just been written down is the work most worth insuring. So [`session end`](/rigger/reference/session/) takes a copy when the newest one predates today:

```console
$ rigger session end rigger
Session on rigger closed, open since 2026-09-15T08:02:11Z.
...
copied the database to ...\rigger.v18-20260915-120253.bak
```

A second sitting on the same day does not take another: today's copy is insurance enough. A copy that cannot be written is reported, but does not fail the close - the session has already ended, and an error there would invite a second `end` on a record that has none open.

## Before a migration, automatically

A migration rewrites the record. So when a newer rigger opens a database written by an older one, it takes this same copy first and says where it went:

```console
$ rigger doctor
Migrating schema 1 -> 2; the previous database is saved as ...\rigger.v1-20260904-084102.bak
```

A fresh database has nothing to lose and is not copied.

## How old the newest copy is

[`doctor`](/rigger/reference/doctor/) prints the age of the newest copy, and says so when it is old:

```console
$ rigger doctor
...
backup:    3 days ago, 5 copies kept - older than a day; run `rigger backup`
```

A copy from today passes without advice. Older than a day is worth a line; older than a week means the copy no longer resembles the record. No copy at all is named loudest: one file, and nothing behind it.

## Restoring

A backup is an ordinary SQLite database. To go back to one, close anything using rigger and copy the file over `rigger.db` - the path [`doctor`](/rigger/reference/doctor/) prints.

## Related

- [`doctor`](/rigger/reference/doctor/) - where the database is, and how old its newest copy is.
- [`session`](/rigger/reference/session/) - the sitting whose end takes a copy.
- [`import`](/rigger/reference/import/) - the command most worth a backup first.
