---
title: Your first week
description: From an empty machine to a project with a plan, a sitting, a release and a record that answers questions - command by command.
---

This is the whole loop once, on a project with no history: plan a stage, work a sitting, ship, and ask the record what happened. Every transcript here was taken from a real run. Install first - [Getting Started](/rigger/getting-started/) has the one line for your platform.

## Day one: a project and a plan

`init` makes the record - one SQLite file in the platform's data directory - and `project add` points it at a repository. The name is the directory's; `--name` overrides.

```console
$ rigger init
Created C:\Users\you\AppData\Local\lacodda\rigger\data\config.toml with the 'line' profile
Created C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db (schema version 27)
Next: rigger project add <path>

$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git
```

A plan is a list of stages, and a stage is a version that will end in a tag. [`version add`](/rigger/reference/version/#version-add) plans one with its tasks, and `--week` aims it at a week of the calendar:

```console
$ rigger version add sample v0.1.0 --title "First light" --task "read a sheet" --task "write the report" --week 2026-W43
Added v0.1.0 to sample, with 2 tasks
  aimed at 2026-W43 - the week of 2026-10-23

$ rigger version add sample v0.2.0 --title "Charts" --task "draw a chart"
Added v0.2.0 to sample, with 1 task
```

What you decide and what only you can answer go into the record as you go, not into a notes file afterwards. [`note`](/rigger/reference/note/) takes a kind - a decision, a finding, a pitfall, a question for the owner:

```console
$ rigger note sample "Read sheets with one library, not two" --kind decision
Recorded a decision for sample

$ rigger note sample "Which file formats come first?" --kind question
Asked the owner about sample; it waits in the inbox until answered
```

## Day two: a sitting

A coding assistant starts from the [context packet](/rigger/concepts/context-packet/) - the stage being built, what waits for you, what happened lately - held to a token budget however long the history behind it grows:

```console
$ rigger context sample
# sample

C:\dev\sample
https://github.com/acme/sample.git
Nothing shipped yet
2 versions planned, 3 tasks open

## Current stage: v0.1.0 · First light
- read a sheet
- write the report

## Waiting for the owner
- [2] Which file formats come first?

## Recent
- 2026-10-07 · decision · Read sheets with one library, not two
```

[`rigger open sample`](/rigger/reference/open/) starts the assistant in the project with that packet as its first message. Connected to the [MCP server](/rigger/reference/mcp/) - the installer registers it - the assistant reads and writes the record itself; everything below is what it does through its tools, shown as the commands a person would type.

A [session](/rigger/reference/session/) holds a sitting together: what is recorded between `start` and `end` belongs to it.

```console
$ rigger session start sample
Session open on sample. Everything recorded now belongs to it.

$ rigger note sample "A merged cell is read as its first cell" --kind finding
Recorded a finding for sample
```

Tasks are closed by their id, which `version show` lists:

```console
$ rigger version show sample
sample v0.1.0 · First light

aimed at   2026-W43 - releases on 2026-10-23
tasks      2 open of 2
  [1] new     read a sheet
  [2] new     write the report

$ rigger task status 1 done
Task 1 is now done (was new): read a sheet

$ rigger task status 2 done
Task 2 is now done (was new): write the report
```

## Day three: a release

The plan does not decide what shipped - git does. Tag the release as you always would, and [`sync`](/rigger/reference/sync/) reads it: the tag closes the version, on the date of its commit, and the conventional commit messages since the last tag become the changelog.

```console
$ git tag v0.1.0
$ rigger sync sample
sample:
  shipped    v0.1.0 on 2026-10-07
  read       2 changes from commit messages
```

The next sitting starts from one line, and the end of this one says what it held:

```console
$ rigger note sample "Start the charts from the report's table" --kind next
Set the next step for sample; the next session starts from it

$ rigger session end sample
Session on sample closed, open since 2026-10-07T13:14:59Z.

recorded 1 finding
closed 2 tasks
next: Start the charts from the report's table
copied the database to C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.v27-20261007-131500.bak

Write it into a diary with: rigger session end sample --diary <file>
```

## The rest of the week: what the record answers

What waits on you, across every project at once - and the answer becomes a decision:

```console
$ rigger inbox
1 question in 1 project

sample       [  2] 2026-10-07  Which file formats come first?

Answer one with: rigger resolve <project> <id> "<answer>"

$ rigger resolve sample 2 "CSV and XLSX; ODS later"
Answered [2]: Which file formats come first?
  the answer is recorded as a decision
```

The project's own screen, for you rather than for the assistant:

```console
$ rigger show sample
sample
Reads every spreadsheet you have

path       C:\dev\sample
remote     https://github.com/acme/sample.git

Last shipped v0.1.0 on 2026-10-07
1 version planned, 1 task open

Current stage: v0.2.0 · Charts
    draw a chart

Next step
  Start the charts from the report's table
```

Where a thing was said, and what went into a release:

```console
$ rigger find sheet
sample       2026-10-07  decision  Read sheets with one library, not two
sample       2026-10-07  change    feat: read a sheet

$ rigger why sample v0.1.0
v0.1.0 · First light — shipped 2026-10-07
the work from the start of the record

2026-10-07  decision  Read sheets with one library, not two
2026-10-07  answered  Which file formats come first?
2026-10-07  finding   A merged cell is read as its first cell
2026-10-07  change    feat: read a sheet
2026-10-07  change    feat: write the report
```

And the calendar, comparing what was aimed at with what the tags say happened - this release came two weeks before its week:

```console
$ rigger calendar --from 2026-W41 --weeks 4
         2026-W41*  2026-W42  2026-W43  2026-W44
sample   <v0.1.0

+ shipped as planned   < early   > slipped   ! overdue   * unplanned   · planned

sample   v0.1.0 — aimed at 2026-W43, 2 weeks early
```

On Monday, [`rigger week`](/rigger/reference/week/) gathers the focus, what ships on Friday and what waits on you into one screen; [`rigger digest`](/rigger/reference/digest/) says what moved, five lines per project.

## For a program rather than a person

Every command above takes `--json` and prints one JSON document, and each command's page has a table of its fields - the [JSON output](/rigger/reference/commands/#json-output) contract a script or an editor can build on.

## Next

- Notes you already keep by hand: [move a hub into the record](/rigger/guides/from-a-hub/).
- Several repositories at once: [`adopt`](/rigger/reference/adopt/).
- At work, tasks are tickets across repositories: [profiles](/rigger/concepts/profiles/) and [task cards](/rigger/reference/task/).
