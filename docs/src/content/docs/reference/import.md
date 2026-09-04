---
title: import
description: Read a notes hub into versions, tasks and events.
---

```
rigger import <PROJECT> --hub <DIR> [--json]
```

Reads a project's notes hub - the plan, the changelog and the decision log - and records what it finds. This is how a project that has been run by hand for months arrives in the database with its history intact.

```console
$ rigger import sample --hub C:\dev\sample\hub
sample:
  versions   38 added, 0 updated
  tasks      111 added, 0 updated
  decisions  9 added
  questions  2 added
```

## What it reads

| File | What comes out of it |
| --- | --- |
| `План.md` | open stages and their tasks; the questions listed under "Ждёт решения владельца" |
| `Изменения.md` | stages that shipped, with the date in their heading |
| `Решения.md` | one decision per dated heading, body and all |

A stage is any heading whose first word is a version - `v0.4.0` - at whatever heading level, so both `## v0.4.0 · Title` and `### v0.4.0 · Title` are found. The title is what follows the version up to the em dash; the tail after it is bookkeeping and is not part of the title. A date anywhere in that tail marks the stage as shipped, written `2026-09-03` or `03.09.2026`, after any of the words the hubs use.

A checkbox line under a stage is one of its tasks, until the next heading at the same level or above - so a backlog list that follows a stage does not become part of it.

## Running it twice

Importing an unchanged hub again changes nothing and says so:

```console
$ rigger import sample --hub C:\dev\sample\hub
sample: nothing changed
```

A stage is identified by its version, a task by its text within its stage, a decision by its date and body, and a question by its text. So a stage that moves from the plan to the changelog when it ships is updated, not duplicated; and a line dropped from the plan stays in the record as the task it was, because an import never deletes.

## When a file is missing

A hub without one of the three files is not an error - the other two are still read, and the missing ones are named:

```console
$ rigger import sample --hub C:\dev\sample
note: План.md is missing from C:\dev\sample
note: Изменения.md is missing from C:\dev\sample
sample: nothing changed
```

## Related

- [`project`](/rigger/reference/project/) - record the project before importing its hub.
- [`backup`](/rigger/reference/backup/) - copy the database aside before a large import.
- [`doctor`](/rigger/reference/doctor/) - what the record holds afterwards.
