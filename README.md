<p align="center"><img src="https://github.com/lacodda/rigger/raw/main/assets/banner.svg" alt="rigger - one seat for every project and task" width="720"></p>

> One seat for all your projects and tasks: a local record of what is done, what is next and when it ships - read by you and your coding assistant.

<p align="center">
  <a href="https://crates.io/crates/rigger"><img src="https://img.shields.io/crates/v/rigger?style=flat-square" alt="crates.io"></a>
  <a href="https://www.npmjs.com/package/@lacodda/rigger"><img src="https://img.shields.io/npm/v/@lacodda/rigger?style=flat-square" alt="npm"></a>
  <a href="https://github.com/lacodda/rigger/actions"><img src="https://img.shields.io/github/actions/workflow/status/lacodda/rigger/ci.yml?style=flat-square" alt="CI"></a>
  <a href="https://github.com/lacodda/rigger/blob/main/LICENSE"><img src="https://img.shields.io/github/license/lacodda/rigger?style=flat-square" alt="License"></a>
</p>

## Why

Run more than a handful of projects and the record of them scatters: a plan in one file, a changelog in another, a session log the assistant writes for itself, a release calendar kept by hand. You stop reading it, because prose cannot be filtered or summed up across projects. The assistant reads it every session at full price. And what actually shipped lives in git, where nobody looks.

rigger keeps the record as data, in one local SQLite file, and derives the rest from it: the packet an assistant starts from, the questions waiting for you, the calendar of what ships when - with git, not the plan, deciding what shipped.

## A day with rigger

Plan a stage, and write down what was decided and what only you can answer:

```console
$ rigger version add sample v0.1.0 --title "First light" --task "read a sheet" --task "write the report" --week 2026-W43
Added v0.1.0 to sample, with 2 tasks
  aimed at 2026-W43 - the week of 2026-10-23

$ rigger note sample "Which file formats come first?" --kind question
Asked the owner about sample; it waits in the inbox until answered
```

A session starts from the packet - a few hundred tokens, whatever the size of the history behind it - rather than from the notes:

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

Through the [MCP server](https://lacodda.github.io/rigger/reference/mcp/) the assistant writes back as it works - decisions, findings, the next step. A tag is the proof a version shipped, and the commits since are the changelog:

```console
$ rigger sync sample
sample:
  shipped    v0.1.0 on 2026-10-07
  read       2 changes from commit messages

$ rigger session end sample
Session on sample closed, open since 2026-10-07T13:14:59Z.

recorded 1 finding
closed 2 tasks
next: Start the charts from the report's table
copied the database to C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.v27-20261007-131500.bak

Write it into a diary with: rigger session end sample --diary <file>
```

And the record answers what the notes never could - what waits on you across every project, where a thing was decided, what went into a release:

```console
$ rigger inbox
1 question in 1 project

sample       [  2] 2026-10-07  Which file formats come first?

Answer one with: rigger resolve <project> <id> "<answer>"

$ rigger resolve sample 2 "CSV and XLSX; ODS later"
Answered [2]: Which file formats come first?
  the answer is recorded as a decision

$ rigger why sample v0.1.0
v0.1.0 · First light — shipped 2026-10-07
the work from the start of the record

2026-10-07  decision  Read sheets with one library, not two
2026-10-07  answered  Which file formats come first?
2026-10-07  finding   A merged cell is read as its first cell
2026-10-07  change    feat: read a sheet
2026-10-07  change    feat: write the report
```

The whole first week, command by command: [Your first week](https://lacodda.github.io/rigger/guides/first-week/).

## What you get

- **Projects, versions, tasks, sessions.** A version is a stage that ends in a tag; a task is work inside it - or, at work, a ticket across several repositories and branches.
- **Facts from git.** A tag means the version shipped, on that date; the plan cannot claim more than git confirms, and `doctor` names where they part.
- **An MCP server as the assistant's only pen**, and a context packet held to a token budget.
- **Your inbox, a weekly brief and a release calendar**, derived from the same record rather than kept by hand.
- **`--json` on every command**, each field held by a test to its reference page.
- **Hubs as an export.** Markdown notes are written from the record, and a hand-kept hub [moves into it](https://lacodda.github.io/rigger/guides/from-a-hub/) in one command.

## Status

In daily use across a line of eighteen products: the record, the packet, the MCP server, facts from git, the owner's screens, task cards at work, and a JSON contract for every command ahead of 1.0. Released versions and what landed in each: [CHANGELOG](https://github.com/lacodda/rigger/blob/main/CHANGELOG.md).

## Install

```powershell
irm https://raw.githubusercontent.com/lacodda/rigger/main/tools/install.ps1 | iex
```

```bash
curl -fsSL https://raw.githubusercontent.com/lacodda/rigger/main/tools/install.sh | sh
```

```bash
npm i -g @lacodda/rigger
cargo install rigger
```

Every installer also leaves `rgr` beside `rigger` - the same program under a shorter name, as a link rather than a second copy. It is skipped when `rgr` already means something else on your machine, and `RIGGER_NO_ALIAS=1` turns it off. `cargo install` produces `rigger` only.

## Documentation

https://lacodda.github.io/rigger/ - getting started, guides, concepts, and a reference page per command. Architecture decisions live in https://github.com/lacodda/rigger/tree/main/docs/adr.

## License

MIT (c) [Kirill Lakhtachev](https://lacodda.com)
