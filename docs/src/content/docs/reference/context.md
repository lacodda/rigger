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
1 version planned, 2 tasks open
Gate: cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test

## Current stage: v0.3.0 · Search
- full-text index
- a query language

## Waiting for the owner
- [2] Pick the release day.

## Wishes, not yet sorted
- [5] (charts asks) Read ODS too (+82 chars)

## Recent
- [3] 2026-09-03 · decision · The record is the database — Prose cannot be filtered.

(Any [id] above reads whole with `rigger event sample <id>`.)

## Next step
Ship the importer next.
```

## The budget

The packet has a token budget - 3000 by default, `--budget` to change it - and holds it. What a session cannot start without comes first and is never dropped: the state line, the current stage with its open tasks, what the project has written down, and the next step. The owner's queue and the recent events share what is left, and the packet says what it left out of either:

```
(16 more wishes left out by the budget; `--explain` names them)
(14 events left out by the budget - `--explain` names them - and 226 more are beyond the window)
```

Those lines are the point of the budget. A packet that quietly ended its list would look like a project where nothing else ever happened.

### One line per question and wish

A question or a wish is printed as the line it is known by - its bold heading when it opens with one, else its first sentence, cut at 120 characters - followed by how much more there is when that is more than a few words:

```
- [12] Tiers for the projects that have none (+455 chars)
```

A wish is written to be acted on cold, so it runs to paragraphs: what it is, where, how to fix it. Printed whole, forty of them came to twice the budget on a real record, and every session on that project started without a single recent event. A session needs to know what is queued, not to read the queue; the one it picks up it reads whole with [`event`](/rigger/reference/event/), by the id in brackets.

### The queue's share

The questions and the wishes may take what the events do not need, and never less than half of what is left after the fixed sections - so a long queue cannot crowd out what was decided lately, and a short history does not leave the budget idle. Within its share the queue is paid for in this order:

1. the questions waiting for you, oldest first;
2. the wishes a neighbour placed, oldest first - somebody is waiting on each;
3. the rest of the wishes, newest first - the newest is the tail of the work just done.

The first line that does not fit ends the queue, so what is left out is always the end of that order. Each list is still printed in the order it was recorded. Questions left out are counted with a pointer to [`inbox`](/rigger/reference/inbox/), which lists every one; wishes left out are named by `--explain`.

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

Long events are summarised rather than truncated at a fixed width: the packet keeps the heading and the first sentence of the reasoning, then says how many characters remain. A decision in a real hub runs to fifteen hundred characters, and three of them at full length would crowd out a dozen others. Every line carries its id, and [`event`](/rigger/reference/event/) reads the rest.

## `--explain`

Shows what each section costs, so an over-budget packet can be understood rather than guessed at:

```console
$ rigger context sample --explain
...
## Cost
state             32 tokens
current stage     18 tokens
questions          7 tokens
wishes            12 tokens
events            21 tokens
documents          9 tokens
next step          6 tokens
total            108 tokens of 3000

## Left out
2026-09-12  wish        24t  [9] Export the report as PDF — the queue's share of the budget is spent
2026-09-17  decision   244t  [41] The address of the action is in the shortcut, not the button… — over the budget
2026-09-16  pitfall    197t  [38] Backticks in an argument are run by the shell and spoil the record.… — over the budget
```

It also names what the budget refused, one line each with its id and the reason. A count tells a session that something is missing; only the names let it go and get the one it needs - with [`event`](/rigger/reference/event/) by its id, or [`find`](/rigger/reference/find/) and [`why`](/rigger/reference/why/) for the story around it. The list is not carried in an ordinary packet: an account of the budget would be spent out of the budget.

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

The same packet as data, for a tool that renders it or an editor that feeds it to a model. Every field of the text form is there, plus the counts of what was left out, and `dropped` when `--explain` filled it. A question or a wish is its `head` and the number of characters `more` the whole runs to, as in the text form; [`event`](/rigger/reference/event/) reads it whole.

<!-- json: context -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The project's name. |
| `state` | object | Where the project stands. |
| `state.path` | string | The project's directory. |
| `state.kind` | string | `repo`, `folder` (a directory of work with no repository - git is never asked, and the text form says so under the path) or `service`. |
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
| `state.retired` | object | When and why the project was [retired](/rigger/reference/project/#project-retire). Absent while it is in the work. |
| `state.retired.since` | string | When it was retired, an RFC 3339 UTC moment. |
| `state.retired.decision` | integer or null | The id of the decision that retired it, which `rigger event` reads. |
| `state.retired.head` | string or null | That decision's first line. |
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
| `questions[].id` | integer | The question's id; `rigger event` reads it whole. |
| `questions[].head` | string | The line it is known by: its bold heading, or its first sentence. |
| `questions[].more` | integer | How many characters longer the whole is than `head`. |
| `questions[].due` | string | The day an answer is needed by, `YYYY-MM-DD`. Absent when no day was set. |
| `questions_omitted` | integer | How many questions the budget left out - the newest. |
| `wishes` | array | Wishes not yet sorted into the plan, the ones the budget kept. |
| `wishes[]` | object | One wish. |
| `wishes[].id` | integer | The wish's id; `rigger event` reads it whole. |
| `wishes[].head` | string | The line it is known by: its bold heading, or its first sentence. |
| `wishes[].more` | integer | How many characters longer the whole is than `head`. |
| `wishes[].asked_by` | string | The neighbour project that asked for it. Absent for the owner's own wishes. |
| `wishes_omitted` | integer | How many wishes the budget left out. |
| `events` | array | Recent events the budget kept, newest first. |
| `events[]` | object | One event. |
| `events[].id` | integer | The event's id; `rigger event` reads it whole. |
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
| `dropped` | array | Only with `--explain`: the questions, wishes and events the budget left out, in the order it refused them. |
| `dropped[]` | object | One line left out. |
| `dropped[].id` | integer | Its id; `rigger event` reads it whole. |
| `dropped[].kind` | string | Its kind: `question`, `wish`, `decision`, `finding`, `pitfall`, `change` and the rest. |
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

- [`event`](/rigger/reference/event/) - reading one line of the packet whole.
- [`find`](/rigger/reference/find/) - searching the events this packet samples from.
- [`why`](/rigger/reference/why/) - the whole story behind one version.

- [`note`](/rigger/reference/note/) - record what this session found, decided or is leaving for the next one.
- [`import`](/rigger/reference/import/) - fill the record from a hub before the first packet.
- [`gate`](/rigger/reference/gate/) - running the command the packet prints on the `Gate:` line.
