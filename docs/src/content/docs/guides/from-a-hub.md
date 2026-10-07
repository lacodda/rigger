---
title: Moving a hub into the record
description: Take a project's hand-kept notes - plan, changelog, decisions, wishes - into the record, let git settle what shipped, and keep the hub as an export.
---

A hub is the folder of markdown notes a project was run from before rigger: a plan, a changelog, a decision log, a list of wishes, a diary of sittings. rigger began as the record of such hubs, and reads them as they are. This guide moves one in, checks it against git, and turns the hub from the place you write into a view the record writes.

## What a hub holds

The hub format is the one the line rigger was built in kept, file names and all:

| File | What it becomes |
| --- | --- |
| `План.md` | the open stages and their tasks; the questions under "Ждёт решения владельца" |
| `Изменения.md` | the stages that shipped, with their dates |
| `Решения.md` | one decision per dated heading |
| `Дневник.md` | one finished session per entry |
| `Хотелки.md` | wishes, taken in once |
| `Видение.md`, `Ритуалы.md`, `Исследования/*.md` | documents, kept whole |

A stage is any heading whose first word is a version (`## v0.2.0 · Charts`), and a task is a checkbox under it. [`import`](/rigger/reference/import/#what-it-reads) has the details. A project without such a hub needs none of this: [`version add`](/rigger/reference/version/#version-add) plans a stage directly.

## Rehearse, then import

Record the repository, and ask what an import would change. `--check` runs the whole import and takes it all back:

```console
$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git

$ rigger import sample --check --hub C:\dev\notes\sample
note: README.md is missing from C:\dev\notes\sample
note: Дневник.md is missing from C:\dev\notes\sample
sample - what an import would change; nothing was written:
  versions   3 added, 0 updated
  tasks      3 added, 0 updated
  decisions  1 added
  wishes     1 taken out of Хотелки.md
  documents  1 added, 0 updated
```

A missing file is a note, not an error - not every hub keeps a diary. When the numbers look right, import for real; running it again later is safe, and an unchanged hub changes nothing:

```console
$ rigger import sample --hub C:\dev\notes\sample
note: README.md is missing from C:\dev\notes\sample
note: Дневник.md is missing from C:\dev\notes\sample
sample:
  versions   3 added, 0 updated
  tasks      3 added, 0 updated
  decisions  1 added
  wishes     1 taken out of Хотелки.md
  documents  1 added, 0 updated
```

A directory of repositories with a directory of hubs beside it moves in one command - [`adopt`](/rigger/reference/adopt/), which takes `--check` too.

## Let git settle what shipped

A changelog says what was meant to ship; a tag says what did. [`sync`](/rigger/reference/sync/) reads the tags and the commits, and where the two disagree the record follows git:

```console
$ rigger sync sample
sample:
  shipped    v0.1.0 on 2026-10-07
  read       2 changes from commit messages
  activity   1 commit since v0.1.0, last on 2026-10-07
```

What git cannot confirm is reported, never corrected: [`doctor`](/rigger/reference/doctor/#where-the-plan-and-git-disagree) lists the versions the hub closed without a tag, and the places where the plan and the tags look one step apart. A clean hub reports nothing:

```console
$ rigger doctor
profile:   line (C:\Users\you\AppData\Local\lacodda\rigger\data\config.toml)
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db
schema:    version 27
projects:  1
versions:  3
tasks:     3
sessions:  0
events:    4
mcp:       answers - protocol 2025-06-18, 24 tools, 1 prompt
backup:    none - one file, no copy of it; run `rigger backup`
```

The packet now carries what the hub held - the stage, the wish waiting to be sorted, the decision, the document:

```console
$ rigger context sample
# sample

C:\dev\sample
https://github.com/acme/sample.git
Last shipped: v0.1.0 on 2026-10-07
2 versions planned, 2 tasks open
1 commit since the last release, the last one today

## Current stage: v0.2.0 · Charts
- label the axes

## Wishes, not yet sorted
- [2] **05.09.2026 · Read ODS too**

Some of the sheets arrive from LibreOffice.

## Recent
- 2026-10-07 · change · fix: keep merged cells
- 2026-10-07 · change · feat: read a sheet
- 2026-09-04 · decision · One library for every format — Two readers drifted apart within a week.

## Written down
- decisions · Решения · 2026-10-07 — `rigger doc show sample decisions`
```

## Hand the hub over

From here on the record is where things are written: through the assistant's [MCP tools](/rigger/reference/mcp/), [`note`](/rigger/reference/note/), [`wish`](/rigger/reference/note/), [`version add`](/rigger/reference/version/#version-add) and [`doc`](/rigger/reference/doc/). The hub becomes an export of it. A hub kept by hand is not overwritten because a command was typed - see what would change, then hand the files over once, on purpose:

```console
$ rigger export sample --check --hub C:\dev\notes\sample
  План.md        would change 171 bytes
  Изменения.md   would change 173 bytes
  Дневник.md     would change 57 bytes
  README.md      would change 57 bytes
...
4 of 4 files differ from the record.

$ rigger export sample --adopt --hub C:\dev\notes\sample
  План.md        written      171 bytes
  Изменения.md   written      173 bytes
  Дневник.md     written      57 bytes
  README.md      written      57 bytes

sample wrote 4 of 4 files.
```

Every file the record owns now opens with `<!-- generated by rigger; edits here are overwritten -->`. A plain `export` keeps them current, and writes only what changed:

```console
$ rigger version add sample v0.3.0 --task "write a CSV"
v0.3.0 was in the plan already; 1 task added

$ rigger export sample --hub C:\dev\notes\sample
  План.md        written      189 bytes
  Изменения.md   unchanged    173 bytes
  Дневник.md     unchanged    57 bytes
  README.md      unchanged    57 bytes

sample wrote 1 of 4 files.
```

The handwritten texts - vision, rituals, research - stay yours: `--docs` writes them back out, `import` takes an edit to them in, and [`doctor --hubs`](/rigger/reference/doctor/) says when a file and the record have parted. The wishes file is read once; a wish left in it afterwards is named by `doctor`, and a new wish goes through [`wish`](/rigger/reference/note/).

## When the hub is no longer read

Once nothing reads the hub - the packet, the inbox and the calendar all come from the record - it is a cache. Keep exporting it while you like reading markdown; delete it when you do not. Nothing in it is anywhere else but the record.
