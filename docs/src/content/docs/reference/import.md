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
| `Решения.md` | one decision per dated heading, body and all; the prose above the first of them becomes the `decisions` [document](/rigger/reference/doc/) |
| `Видение.md` | the `vision` document |
| `Ритуалы.md` | the `rituals` document |
| `Исследования/*.md` | one `research` document each |

A stage is any heading whose first word is a version - `v0.4.0` - at whatever heading level, so both `## v0.4.0 · Title` and `### v0.4.0 · Title` are found. The title is what follows the version up to the em dash; the tail after it is bookkeeping and is not part of the title. A date anywhere in that tail marks the stage as shipped, written `2026-09-03` or `03.09.2026`, after any of the words the hubs use.

A checkbox line under a stage is one of its tasks, until the next heading at the same level or above - so a backlog list that follows a stage does not become part of it.

### The handwritten texts

The first five files are parsed into versions, tasks and events. The last four are not parsed at all: they are taken in whole, as [documents](/rigger/reference/doc/), because they are the files a hub kept that the record could not rebuild - and the reason a hub had to exist at all.

```console
$ rigger import rigger --hub ~/vault/Projects/rigger
rigger:
  versions   2 added, 11 updated
  documents  4 added, 0 updated
```

A research note is addressed by the date its filename opens with. Two notes from one day get a number after it, so neither is lost: the titles after the date are usually not typeable as an address.

Re-reading an unchanged hub changes nothing, so a document edited with `doc edit` and not yet exported is not silently replaced by the older file it came from. A file that really changed is taken in and reported as updated.

## Running it twice

Importing an unchanged hub again changes nothing and says so:

```console
$ rigger import sample --hub C:\dev\sample\hub
sample: nothing changed
```

A stage is identified by its version, a task by its text within its stage, a decision by its date and body, and a question by its text. So a stage that moves from the plan to the changelog when it ships is updated, not duplicated.

## What the hub struck

A hub kept by hand is the owner's pen, and a line struck from it is struck. A task the plan no longer lists is marked `dropped`: the export does not write it, the packet does not count it, and it comes back as open the moment the plan lists it again. A planned stage that neither file names any more - renumbered when the queue moved, or given up - is dropped the same way, with its open tasks. Done tasks and shipped stages stay: they are history, and a tag outranks a plan that has not been told.

```console
$ rigger import sample --hub C:\dev\sample\hub
sample:
  tasks      2 added, 1 updated
  dropped    1 version and 4 tasks struck from the hub
```

Nothing is deleted. A dropped row keeps its id and its history, and reads as what it is.

## What a stale plan cannot do

A plan that has not been told a stage shipped is behind, not a decision. Its empty boxes do not reopen the tasks the record closed, and its depth and place do not follow the stage into the changelog. Its copy of a shipped stage - a line in the major map - neither un-ships it nor writes over the changelog's entry.

## The diary

`Дневник.md` is read into sittings: one entry per heading, with the heading exactly as written. An entry read once by an older reader - its date kept apart from its heading - is the same entry when read again, not a second one; the record keeps one row and the heading the hub writes now.

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
