<p align="center"><img src="https://github.com/lacodda/rigger/raw/main/assets/banner.svg" alt="rigger - one seat for every project and task" width="720"></p>

> One seat for all your projects and tasks: a local record of what is done, what is next and when it ships - read by you and your coding assistant.

<p align="center">
  <a href="https://crates.io/crates/rigger"><img src="https://img.shields.io/crates/v/rigger?style=flat-square" alt="crates.io"></a>
  <a href="https://www.npmjs.com/package/rigger-cli"><img src="https://img.shields.io/npm/v/rigger-cli?style=flat-square" alt="npm"></a>
  <a href="https://github.com/lacodda/rigger/actions"><img src="https://img.shields.io/github/actions/workflow/status/lacodda/rigger/ci.yml?style=flat-square" alt="CI"></a>
  <a href="https://github.com/lacodda/rigger/blob/main/LICENSE"><img src="https://img.shields.io/github/license/lacodda/rigger?style=flat-square" alt="License"></a>
</p>

## Why

Run more than a handful of projects and the record of them scatters: a plan in one file, a changelog in another, a session log the assistant writes for itself, a release calendar kept by hand. Three symptoms follow. You stop reading the record, because it is prose and prose cannot be filtered or summed up across projects. The assistant reads it every session at full price, and the biggest project no longer fits in a context window at all. And the facts about what actually shipped live in git, where nobody looks.

rigger keeps the record as data, in a local SQLite file, and derives everything else from it: the five-line digest you read, the context packet the assistant starts from, the calendar of what ships when, and the queue of decisions waiting for you.

## A first look

```console
$ rigger init
Created C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db (schema version 1)
Next: rigger project add <path>

$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git

$ rigger project show sample
sample
  path:    C:\dev\sample
  remote:  https://github.com/acme/sample.git
  since:   2026-09-04T07:41:10Z

$ rigger doctor
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
schema:    version 1
projects:  1
versions:  0
tasks:     0
sessions:  0
events:    0
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

v0.1.0 is the first release: the database, projects and `doctor`. The road to 1.0 is twenty-two small releases in seven blocks:

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
npm i -g rigger-cli
cargo install rigger
```

## Documentation

https://lacodda.github.io/rigger/ - getting started, concepts, and a reference page per command. Architecture decisions live in https://github.com/lacodda/rigger/tree/main/docs/adr.

## License

MIT - see https://github.com/lacodda/rigger/blob/main/LICENSE.
