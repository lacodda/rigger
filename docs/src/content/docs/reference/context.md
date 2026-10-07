---
title: context
description: Print what an assistant needs to start a session on a project.
---

```
rigger context <PROJECT> [--json] [--explain] [--budget <TOKENS>]
```

Prints the context packet: where the project stands, what is being built, what waits for you, what happened lately, and the one line the last session left behind. This is what an assistant reads instead of a project's notes, and it is the reason the record lives in a database.

```console
$ rigger context sample
# sample

C:\dev\sample
https://github.com/acme/sample.git
Last shipped: v0.2.0 on 2026-09-03
1 versions planned, 2 tasks open
Gate: cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test

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

## The budget

The packet has a token budget - 3000 by default, `--budget` to change it - and holds it. Everything a session cannot start without comes first and is never dropped: the state line, the current stage with its open tasks, the questions waiting for you, the wishes not yet sorted, what the project has written down, and the next step. Recent events fill what is left, newest first, and the packet says what it left out:

```
(14 events left out by the budget; `--explain` names them - and 226 more are beyond the window)
```

That line is the point of the budget. A packet that quietly ended its list would look like a project where nothing else ever happened.

## Work put down

A task [asleep](/rigger/reference/task/#putting-work-down) until a day is left out of the stage and counted in its place, and a stage whose every open task is frozen or asleep is passed over for the next planned version and named:

```
## Current stage: v0.4.0 · Export
- write the exporter
(1 task asleep until 2026-10-14)
(set aside: v0.3.0 · Search (1 frozen))
```

Counted rather than dropped for the same reason the budget says what it left out: a stage that silently lost a task reads as a stage with less to do.

The two numbers are different facts and are counted apart. The first is what the budget refused. The second is what is older than the window the packet looks at - two hundred events - which no budget would have reached anyway and which [`find`](/rigger/reference/find/) is for. Adding them made a project with four hundred events of history look as though a session had been denied four hundred of them.

Long events are summarised rather than truncated at a fixed width: the packet keeps the heading and the first sentence of the reasoning, then says how many characters remain. A decision in a real hub runs to fifteen hundred characters, and three of them at full length would crowd out a dozen others.

## `--explain`

Shows what each section costs, so an over-budget packet can be understood rather than guessed at:

```console
$ rigger context sample --explain
...
## Cost
state             32 tokens
current stage     18 tokens
questions          7 tokens
events            21 tokens
documents          9 tokens
next step          6 tokens
total             96 tokens of 3000

## Left out
2026-09-17  decision   244t  The address of the action is in the shortcut, not the button… — over the budget
2026-09-16  pitfall    197t  Backticks in an argument are run by the shell and spoil the record.… — over the budget
```

It also names what the budget refused, one line each with the reason. A count tells a session that something is missing; only the names let it go and get the one it needs - with [`find`](/rigger/reference/find/) or [`why`](/rigger/reference/why/). The list is not carried in an ordinary packet: an account of the budget would be spent out of the budget.

Token counts are estimated from characters, not measured with a tokeniser: the number decides how much history to include, and an estimate that errs high is the safe direction.

## What the project has written down

One line per [document](/rigger/reference/doc/) - its kind, its title, when it last changed, and the command that opens it:

```
## Written down
- vision · Vision of sample · 2026-09-12 — `rigger doc show sample vision`
- rituals · Rituals of sample · 2026-09-15 — `rigger doc show sample rituals`
```

The documents themselves are not in the packet: a vision runs to thousands of characters and would eat it whole. But a session that does not know a vision exists cannot ask for it, and the commonest way to contradict one is not to have heard of it.

## `--json`

The same packet as data, for a tool that renders it or an editor that feeds it to a model. Every field of the text form is there, plus `events_omitted`, `events_beyond_window`, and `dropped` when `--explain` filled it.

<!-- json: context -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The project's name. |
| `state` | object | Where the project stands. |
| `state.path` | string | The project's directory. |
| `state.remote` | string or null | The git remote's URL, when there is one. |
| `state.last_shipped` | string or null | The newest shipped version. |
| `state.last_shipped_on` | string or null | The day it shipped, `YYYY-MM-DD`. |
| `state.delivery_short` | string | How far the newest version got past its tag, when that falls short of installable: `tag-only`, `no-assets`, `released`, `publish-running` or `publish-failed`. Absent when delivery is complete. |
| `state.versions_planned` | integer | How many versions are planned and unshipped. |
| `state.tasks_open` | integer | How many tasks are open. |
| `state.days_quiet` | integer or null | Days since anything was recorded about the project. |
| `state.commits_since_tag` | integer or null | Commits since the newest tag, as the last `sync` read them. |
| `state.days_since_commit` | integer or null | Days since the last commit. |
| `state.since_last_session` | object or null | What happened since the last session ended; `null` when none has. |
| `state.since_last_session.ended_at` | string | When the last session ended, an RFC 3339 UTC moment. |
| `state.since_last_session.days_ago` | integer or null | Days since that moment. |
| `state.since_last_session.events` | integer | Events recorded since, not counting commits. |
| `state.since_last_session.commits` | integer | Changes read from commit messages since. |
| `state.gate` | string or null | The command that says the project is fit to commit. |
| `current` | object or null | The current stage: the first planned version with open work that is not all put down; `null` when there is none. |
| `current.version` | string | The stage's version, like `v0.3.0`. |
| `current.title` | string | The stage's title. |
| `current.tasks` | array | The stage's open tasks that are not put down. |
| `current.tasks[]` | object | One task. |
| `current.tasks[].id` | integer | The task's id. |
| `current.tasks[].status` | string | One of `new`, `active`, `waiting-handoff`, `frozen`, `done`, `dropped`. |
| `current.tasks[].title` | string | The task's title. |
| `current.asleep` | array | Open tasks asleep until a day; counted in the text packet, listed here. |
| `current.asleep[]` | object | One sleeping task. |
| `current.asleep[].id` | integer | The task's id. |
| `current.asleep[].status` | string | The task's status, as in `current.tasks[].status`. |
| `current.asleep[].title` | string | The task's title. |
| `current.asleep[].snoozed_until` | string | The day it wakes, `YYYY-MM-DD`. |
| `current.set_aside` | array | Earlier versions whose open work is all frozen or asleep. |
| `current.set_aside[]` | object | One version set aside. |
| `current.set_aside[].version` | string | The version. |
| `current.set_aside[].title` | string or null | The version's title. |
| `current.set_aside[].frozen` | integer | How many of its open tasks are frozen. |
| `current.set_aside[].asleep` | integer | How many of its open tasks are asleep. |
| `questions` | array | Questions waiting for the owner. |
| `questions[]` | object | One question. |
| `questions[].id` | integer | The question's id. |
| `questions[].text` | string | The question. |
| `questions[].due` | string | The day an answer is needed by, `YYYY-MM-DD`. Absent when no day was set. |
| `wishes` | array | Wishes not yet sorted into the plan. |
| `wishes[]` | object | One wish. |
| `wishes[].id` | integer | The wish's id. |
| `wishes[].text` | string | The wish. |
| `wishes[].asked_by` | string | The neighbour project that asked for it. Absent for the owner's own wishes. |
| `events` | array | Recent events the budget kept, newest first. |
| `events[]` | object | One event. |
| `events[].kind` | string | The event's kind, such as `decision`, `finding`, `pitfall` or `change`. |
| `events[].date` | string | The day it was recorded, `YYYY-MM-DD`. |
| `events[].body` | string | The event's text. |
| `documents` | array | The handwritten texts the project has; named, not included. |
| `documents[]` | object | One document. |
| `documents[].kind` | string | The document's kind, such as `vision` or `rituals`. |
| `documents[].slug` | string | The name `rigger doc show` takes. |
| `documents[].title` | string | The document's title. |
| `documents[].updated_at` | string | When it was last written, an RFC 3339 UTC moment. |
| `next_step` | string or null | The line the last session left behind. |
| `events_omitted` | integer | How many recent events the budget left out. |
| `events_beyond_window` | integer | How many events are older than the window the packet looks at. |
| `dropped` | array | Only with `--explain`: the events the budget left out, newest first. |
| `dropped[]` | object | One event left out. |
| `dropped[].kind` | string | Its kind: `decision`, `finding`, `pitfall`, `change` and the rest. |
| `dropped[].date` | string | The day it was recorded, `YYYY-MM-DD`. |
| `dropped[].head` | string | Its first line, so it can be asked for by name. |
| `dropped[].tokens` | integer | What it would have cost, in estimated tokens. |
| `dropped[].why` | string | Why it was left out. |

## Since the last sitting

When a [session](/rigger/reference/session/) has ended before, the state opens with a line saying where it stopped and what has happened since:

```
Last session ended yesterday: 3 events recorded, 7 changes committed
```

The question a returning assistant has is not "what has been going on" but "what changed while I was away" - and without this line the packet's recent events are undated history that every session re-reads from the top.

## Related

- [`find`](/rigger/reference/find/) - searching the events this packet samples from.
- [`why`](/rigger/reference/why/) - the whole story behind one version.

- [`note`](/rigger/reference/note/) - record what this session found, decided or is leaving for the next one.
- [`import`](/rigger/reference/import/) - fill the record from a hub before the first packet.
- [`gate`](/rigger/reference/gate/) - running the command the packet prints on the `Gate:` line.
