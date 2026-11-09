---
title: inbox
description: Every question waiting for your answer, across all projects.
---

```
rigger inbox [--project <NAME>] [--json]
```

The queue of questions waiting on you, gathered from every project at once. Until this, they were never in one place - they sat in each project's notes, and a question could wait weeks because nobody was looking at that file.

```console
$ rigger inbox
6 questions in 3 projects

alpha        [  1] 2026-09-04  Place in the queue
             [  2] 2026-09-04  Sign the binaries?
beta         [  3] 2026-09-04  Place in the queue
             [  4] 2026-09-04  Sign the binaries?
gamma        [  5] 2026-09-04  Place in the queue
             [  6] 2026-09-04  Sign the binaries?

Asked by several projects - one answer settles each group:
  Place in the queue — alpha, beta, gamma
  Sign the binaries? — alpha, beta, gamma

Answer one with: rigger resolve <project> <id> "<answer>"
```

Grouped by project, because answering is done a project at a time, and oldest first within one - what has waited longest is what is most overdue.

## A question past its day

A question asked with a day ([`note --kind question --due`](/rigger/reference/note/#a-question-with-the-day-it-is-due), or `ask_owner` with `due`) carries it at the end of its line, and once the day has gone the line says so - in red, for a person at a terminal:

```console
$ rigger inbox
2 questions in 1 project, 1 overdue

sample       [  7] 2026-09-18  Which registry goes first?  overdue since 2026-09-21
             [  8] 2026-09-22  Which icon?  due 2026-10-22
```

`--json` carries `due` and `overdue` on every question, read against today when the inbox is read, so every screen that shows a question agrees on whether it is late.

## One answer, several projects

The block at the bottom is the reason this command groups rather than lists. Questions are copied between projects: a real record had three of them waiting on the same decision about a place in the queue, and without the grouping it read as three separate jobs.

Grouping is by wording, not by meaning. rigger notices that two questions are spelt alike - case and punctuation aside - and does not guess at what they have in common. That is enough for the case it exists for: the same question written into several hubs.

A question only one project asks is not a group, and neither are two questions inside one project - the block is about an answer that settles several projects at once.

## Questions come from two places

A hub's "waiting for the owner" section becomes questions when it is [imported](/rigger/reference/import/), and an assistant adds one with the `ask_owner` tool over [MCP](/rigger/reference/mcp/). Both land in the same queue.

## What the neighbours are asking for

Below the questions, the orders one product has placed with another - a [wish recorded with `--from`](/rigger/reference/note/):

```console
Neighbours are asking for:
dowel        [88] 2026-09-18  A status colour that is not the accent — lyrid
```

These are not questions and nothing is being asked of you to decide: they are work one product is waiting on another to do. They are on this screen because it is where you look for what is waiting, and in a group of their own because answering a question and doing a piece of work are not the same job.

One is taken off the list by [`rigger resolve`](/rigger/reference/resolve/), like any wish.

## Tickets taken in kasl without a card

In a [ticket profile](/rigger/reference/profile/), the last line names the tickets you took in kasl and never made a card for - read from kasl's inbox, as [`task incoming`](/rigger/reference/task/#tickets-from-kasls-inbox) reads it:

```console
2 tickets taken in kasl have no card: ACME-2, ACME-5 - rigger task incoming
```

Taken only: a ticket nobody took is not anybody's work yet, and `task incoming` lists those. The one taken longest ago comes first, and past five the rest are counted. A ticket asleep or gone from the tracker is left out, as is one whose card knows it by its key or an alias.

The questions do not need kasl, so the inbox never fails for want of it. With no kasl here it says nothing about tickets; a kasl that cannot list its inbox is one line in place of this one - `kasl did not list its inbox (...): the tickets taken there are not counted`. A line profile does not ask kasl, and neither does `--project`: a ticket is the desk's, not one project's.
## JSON

Under `--json` it prints the groups of the screen.

<!-- json: inbox -->
| Field | Type | Meaning |
| --- | --- | --- |
| `waiting` | array | The open questions, oldest first within a project. |
| `waiting[]` | object | One question. |
| `waiting[].project` | string | The project asked. |
| `waiting[].id` | integer | The question's id, for `rigger resolve`. |
| `waiting[].date` | string | The day it was asked, `YYYY-MM-DD`. |
| `waiting[].body` | string | The question's text. |
| `waiting[].due` | string or null | The day an answer is needed by, `YYYY-MM-DD`; null when it has none. |
| `waiting[].overdue` | boolean | Whether that day has gone by, read against today. |
| `shared` | array | Questions spelt alike in several projects. |
| `shared[]` | object | One group of alike questions. |
| `shared[].subject` | string | The heading the questions share. |
| `shared[].projects` | array | The projects that ask it. |
| `shared[].projects[]` | string | A project name. |
| `asked_by_neighbours` | array | Orders one product has placed with another. |
| `asked_by_neighbours[]` | object | One order. |
| `asked_by_neighbours[].id` | integer | The wish's id, for `rigger resolve`. |
| `asked_by_neighbours[].body` | string | What is asked for. |
| `asked_by_neighbours[].asked_by` | string | The project that asked. |
| `asked_by_neighbours[].project` | string | The project being asked. |
| `asked_by_neighbours[].date` | string | The day it was recorded, `YYYY-MM-DD`. |
| `taken_without_card` | array or null | The tickets taken in kasl that have no card, taken longest ago first; `null` when kasl's inbox was not read - a line profile, `--project`, no kasl here, or a kasl that could not answer. |
| `taken_without_card[]` | object | One ticket. |
| `taken_without_card[].key` | string | The ticket's id in the tracker. |
| `taken_without_card[].summary` | string | The tracker's title for it. |
| `taken_without_card[].status` | string or null | The tracker's status, by name; `null` until kasl knows it. |
| `taken_without_card[].taken_at` | string | When it was taken in kasl, in kasl's local `YYYY-MM-DDTHH:MM:SS`. |
| `kasl_said` | string or null | Why kasl could not list its inbox, when it ran and did not; `null` otherwise. |
## Answering

[`rigger resolve`](/rigger/reference/resolve/) takes a question off the queue, and the answer becomes a decision in the record:

```console
$ rigger resolve alpha 1 "Weight 3, reviewed each cycle."
Answered [1]: **Place in the queue:** no weight assigned yet.
  the answer is recorded as a decision
```

Nothing is deleted - the question stays in the record as what was asked, and only leaves the queue.

## Related

- [`resolve`](/rigger/reference/resolve/) - answering one.
- [`digest`](/rigger/reference/digest/) - the other direction: what moved.
- [`context`](/rigger/reference/context/) - the same questions, in a project's packet.
- [`link`](/rigger/reference/link/) - the ties the neighbours' orders sit beside.
