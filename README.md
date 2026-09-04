<p align="center"><img src="https://github.com/lacodda/rigger/raw/main/assets/banner.svg" alt="rigger - one seat for every project and task" width="720"></p>

> One seat for all your projects and tasks: a local record of what is done, what is next and when it ships - read by you and your coding assistant.

<p align="center">
  <a href="https://crates.io/crates/rigger"><img src="https://img.shields.io/crates/v/rigger?style=flat-square" alt="crates.io"></a>
  <a href="https://www.npmjs.com/package/@lacodda/rigger"><img src="https://img.shields.io/npm/v/@lacodda/rigger?style=flat-square" alt="npm"></a>
  <a href="https://github.com/lacodda/rigger/actions"><img src="https://img.shields.io/github/actions/workflow/status/lacodda/rigger/ci.yml?style=flat-square" alt="CI"></a>
  <a href="https://github.com/lacodda/rigger/blob/main/LICENSE"><img src="https://img.shields.io/github/license/lacodda/rigger?style=flat-square" alt="License"></a>
</p>

## Why

Run more than a handful of projects and the record of them scatters: a plan in one file, a changelog in another, a session log the assistant writes for itself, a release calendar kept by hand. Three symptoms follow. You stop reading the record, because it is prose and prose cannot be filtered or summed up across projects. The assistant reads it every session at full price, and the biggest project no longer fits in a context window at all. And the facts about what actually shipped live in git, where nobody looks.

rigger keeps the record as data, in a local SQLite file, and derives everything else from it: the five-line digest you read, the context packet the assistant starts from, the calendar of what ships when, and the queue of decisions waiting for you.

## A first look

```console
$ rigger init
Created C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db (schema version 4)
Next: rigger project add <path>

$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git

$ rigger import sample --hub C:\dev\sample\hub
sample:
  versions   3 added, 0 updated
  tasks      2 added, 0 updated
  decisions  1 added
  questions  1 added

$ rigger doctor
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
schema:    version 4
projects:  1
versions:  3
tasks:     2
sessions:  0
events:    2
```

Years of notes arrive in one command, and running it again is quiet. Then a session starts from the packet rather than from the notes:

```console
$ rigger context sample
# sample

C:\dev\sample
https://github.com/acme/sample.git
Last shipped: v0.2.0 on 2026-09-03
1 versions planned, 2 tasks open

## Current stage: v0.3.0 · Search
- full-text index
- a query language

## Waiting for the owner
- [2] Pick the release day.

## Recent
- 2026-09-03 · decision · The record is the database — Prose cannot be filtered.

## Next step
Ship the importer next.
```

That packet costs 96 tokens here and holds a 3000-token budget on a project with years of history - against tens of thousands for reading the notes it came from, which past a certain size no longer fit at all.

One command hands it to your assistant and starts the session in the project:

```console
$ rigger open sample
Starting claude in C:\dev\sample with the packet for sample
```

Connect the MCP server instead, and the assistant reads the same packet and writes back to the record as it works - decisions, findings and pitfalls become events, not lines in a transcript nobody opens again:

```console
$ claude mcp add rigger -- rigger mcp
Added stdio MCP server rigger with command: rigger mcp to local config
```

What actually shipped is not taken on trust. `sync` reads the repository's tags and commits in-process and writes what they prove; where the plan says a version shipped and no tag agrees, the disagreement is reported rather than corrected:

```console
$ rigger sync claimed
claimed:
  shipped    v0.1.0 on 2026-09-04
  read       2 changes from commit messages
  no tag     v0.2.0 is closed in the plan
```

Those changes come out of the commit messages themselves - `feat`, `fix` and anything breaking, dated by the commit rather than by the sync - so the chronicle stays current whether or not anyone opens a session.

Once there is a record, it answers questions - which is the point of keeping it as data:

```console
$ rigger find budget
sample       2026-09-04  decision  The budget is a gate, not a suggestion.
sample       2026-09-04  pitfall   A wide window hides what the budget dropped.

$ rigger why sample v0.3.0
v0.3.0 — shipped 2026-09-04
the work after v0.2.0 (2026-09-04)

2026-09-04  decision  The budget is a gate, not a suggestion.
2026-09-04  pitfall   A wide window hides what the budget dropped.
2026-09-04  change    feat: rank what a person wrote above a commit
```

And it answers the question the notes never could - what is waiting on you, across everything at once:

```console
$ rigger inbox
6 questions in 3 projects

alpha        [  1] 2026-09-04  Place in the release calendar
             [  2] 2026-09-04  Sign the binaries?
beta         [  3] 2026-09-04  Place in the release calendar

Asked by several projects - one answer settles each group:
  Place in the release calendar — alpha, beta, gamma
```

A project is named after its directory - the name you call it by, not the one its manifest publishes under - and `--name` overrides. Every command that shows facts also prints them with `--json`.

## What it will do

- **Projects, versions, tasks, sessions.** A project has a map of versions; a version is a stage that ends in a tag; a task is a unit of work inside a version (at home) or a ticket across several projects and branches (at work). One model, two profiles.
- **Facts from git.** A pushed tag means the version shipped, on that date. Commits since the last tag are activity. The plan cannot claim more than git confirms.
- **A context packet instead of a hub.** `rigger context <project>` prints what the assistant needs to start - state, current stage, open questions, last events, next step - within a fixed token budget.
- **An MCP server as the assistant's only pen.** Decisions, findings, pitfalls, changes and the next step are recorded as events through tools, not by editing markdown.
- **The owner's inbox.** Every question waiting for your answer, across all projects, in one list.
- **A release calendar** with tiers and rhythm, derived from the same data.
- **Hubs as an export.** The markdown files you keep in Obsidian are generated from the database, not written by hand.

## Status

v0.9.0 opens the first two screens meant for you rather than the assistant: `inbox` gathers every question waiting on your answer across all projects - grouping the ones several projects are asking at once - and `digest` says what moved, five lines per project. Before them: the database, `import`, the context packet, the `mcp` server, facts from git, a chronicle that writes itself, and search. The road to 1.0 is twenty-two small releases in seven blocks:

| Block | Versions | What it delivers |
| --- | --- | --- |
| Record | 0.1 - 0.5 | database and projects, hub import, the context packet, a Claude Code session from it, the MCP server |
| Facts | 0.6 - 0.8 | tags and commits as truth, changes read from commits, search and "why" |
| Owner | 0.9 - 0.12 | inbox and digest, the release calendar, the Monday brief, retro |
| Sessions | 0.13 - 0.14 | sessions with an end-of-session journal, hubs exported from the database |
| Line | 0.15 - 0.16 | thin project skills, every project of a line recorded at once |
| Tasks | 0.17 - 0.21 | the ticket profile, task cards, activity from branches, incoming material, snooze and handoff |
| Stabilisation | 0.22 | `--json` everywhere, docs, a signed binary where needed |

**1.0 "Truth in the database":** a full week where every session on every project runs through rigger and no hub is edited by hand. A desktop window (Tauri) and code knowledge follow in 1.x.

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

Every installer also leaves `rgr` beside `rigger` - the same program under a shorter name, as a link rather than a second copy, so it cannot fall behind. It is skipped when `rgr` already means something else on your machine, and `RIGGER_NO_ALIAS=1` turns it off. `cargo install` produces `rigger` only.

## Documentation

https://lacodda.github.io/rigger/ - getting started, concepts, and a reference page per command. Architecture decisions live in https://github.com/lacodda/rigger/tree/main/docs/adr.

## License

MIT - see https://github.com/lacodda/rigger/blob/main/LICENSE.
