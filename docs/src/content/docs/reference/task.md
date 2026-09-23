---
title: task
description: Task cards - the desk's unit of work - and the status of any task.
---

```
rigger task new <TITLE> [--id <KEY>] [--alias <TEXT>]... [--project <NAME>]... [--branch <BRANCH>] [--summary <TEXT>]
rigger task find <QUERY> [--json]
rigger task open <CARD>
rigger task active [--json]
rigger task close [<CARD>] [--status <STATUS>]
rigger task show <CARD> [--json]
rigger task context <CARD> [--budget <TOKENS>]
rigger task link <CARD> <PROJECT> [--branch <BRANCH>] [--role <TEXT>]
rigger task list [--status open|all|<STATUS>] [--json]
rigger task rename <CARD> <KEY>
rigger task alias <CARD> <TEXT>
rigger task summary <CARD> <TEXT>
rigger task note <CARD> <TEXT> [--kind <KIND>]
rigger task status <TASK> <STATUS>
```

At a desk the unit of work is not a version but a ticket: an id, a title, a branch in each of the repositories it touches, and everything learnt while working it. A **card** is that ticket in the record. It used to be a folder of seven markdown files kept by a script; now it is a row, its links, and the events written against it - and the packet an assistant starts from is composed rather than read aloud.

Cards live in the desk, a project the record keeps for itself, made the first time a card is. `<CARD>` anywhere below is a card's key, one of its aliases, or its id; case does not matter.

## Finding the card a line means

A task arrives as a line of text: `WA-4130`, an id with a title after it, two customer numbers and a description, or just the description. Ids change when a task moves between trackers while the title stays, so the line is looked up by everything it holds - the ids, the numbers and the words - and the answer says how sure it is:

```console
$ rigger task find "WA-4130 rtf files are not supported"
100  WA-4130  new              rtf files are not supported  (id=WA-4130)

take WA-4130: it is the one.
```

| Verdict | When | What to do |
| --- | --- | --- |
| `take` | an id matched, or one card's title is close enough and no other is | take it; ask nothing |
| `ask` | one card is middling, or several are close | show them; the owner picks, or says it is new |
| `new` | nothing is close | make a card |

A duplicate card is worse than one more question, so the middle is wide on purpose. The scoring came from a script that ran a desk for a year: an id match is certain; a title is compared by the Dice coefficient over character bigrams (robust to typos and inflection) and by the share of the query's words found in it (a short query against a long title is not penalised); a customer case number found in the card lifts it. The weak trail is reported too - an id or a number mentioned only in what was written against another card, which is how a task that moved is caught:

```
mentioned 323858: WA-9 - another card
```

## Making and keeping a card

```console
$ rigger task new "rtf files are not supported" --id WA-4130 --project webapp --branch fix/rtf
Made card WA-4130 and opened it: rtf files are not supported
  webapp (C:\work\webapp) on fix/rtf
```

Without `--id` the card gets a local key, `LOCAL-<day>-<n>`, and says so; when the tracker names the task, `task rename` gives the card the id and keeps the old key as an alias - things already refer to it. `task alias` adds another name a task goes by; `task summary` says what it is in one paragraph; `task link` names a repository it is worked in, with the branch and what the repository is to the task.

The card just made is the one **in hand**. `task open` puts another there; `task active` says which; a card left in hand longer than a working day is not in hand any more, so a desk that forgot to close it yesterday is not told today that it is. `task close` closes the card in hand - or the one named - as `done`, or as the `--status` given, and leaves nothing in hand.

## What git says about a card

Since v0.24.0 a card is found in git without anybody saying where it is worked. [`sync`](/rigger/reference/sync/#cards-in-git) reads the branches of every recorded repository - local ones, and a remote's where there is no local one of that name - and the newest 500 commits across all of them, and looks for the names of the cards the record holds: a card's key, and those of its aliases that are ids (`API-241`, the `LOCAL-...` key it had before `task rename`), never an alias that is a phrase. A name counts only as a whole word in any case: `fix/wa-4130-rtf` and `feat(WA-4130): ...` name `WA-4130`; `WA-41300` does not.

What is found is written as facts:

- the repository becomes one the card is **worked in**, and the first branch found becomes the link's branch - unless the link already names one, since a branch given with `task link` is the owner's to change;
- the branches are the repository's state and are read afresh every time, so a branch deleted after its merge leaves the card;
- the commits are history and stay: a card keeps what was done for it after its branch is gone.

An id that names no card makes none. It is somebody else's ticket, or one never handed to this desk.

```console
$ rigger task show WA-4130
WA-4130 · rtf files are not supported
  status:   active
  since:    2026-09-08T09:12:40Z
  worked in:
  webapp (C:\work\webapp) on fix/WA-4130-rtf
  in git:
    webapp - 1 commit names it; last 2026-09-19 be331aa3 fix: keep the rtf body
      branch fix/WA-4130-rtf, tip 2026-09-19
  idle:     4 days without movement
```

The last commit is the newest of the commits that name the card and the tips of its branches: most commits on a ticket's branch do not repeat its id, and the branch moving is the ticket moving. **Idle** is the whole days since then, across every repository the card is worked in. The [packet](#the-packet-of-a-card) carries the same lines under `## In git`.

### The shape of `task show --json`

This is a contract: a git client that makes a worktree per task reads the branch from here, and a test holds every field of it to this page.

```json
{
  "card": { "id": 1, "key": "WA-4130", "title": "...", "status": "active", "aliases": [], "summary": null, "created_at": "...", "updated_at": "..." },
  "links": [{ "project": "webapp", "path": "C:\\work\\webapp", "branch": "fix/WA-4130-rtf", "role": null }],
  "activity": [{
    "project": "webapp",
    "branches": [{ "name": "fix/WA-4130-rtf", "remote": null, "tip": "be331aa3...", "tip_at": "2026-09-19T14:02:11Z" }],
    "commits": 1,
    "last_commit": { "hash": "be331aa3...", "at": "2026-09-19T14:02:11Z", "subject": "fix: keep the rtf body" }
  }],
  "idle_days": 4,
  "events": 0
}
```

| Field | What it holds |
| --- | --- |
| `card` | the card: `id`, `key`, `title`, `status`, `aliases`, `summary`, `created_at`, `updated_at` |
| `links` | the repositories it is worked in: `project`, `path`, `branch` (the one a worktree is made from), `role` |
| `activity` | what git said, per `project`: its `branches`, how many `commits` name the card, and the `last_commit` |
| `branches` | `name` as it is checked out; `remote` when the branch is only on that remote, `null` when it is local; `tip` and `tip_at`, the tip commit and its moment in UTC |
| `last_commit` | `hash`, `at` (UTC) and `subject` of the newest movement |
| `idle_days` | whole days since the newest movement anywhere; `null` when git has said nothing about the card |
| `events` | how many events are written against the card |

## Writing against a card

Everything a session learns goes against the card as events: `rigger task note <CARD> <TEXT> --kind <KIND>`, or the `task` argument of the recording tools over [MCP](/rigger/reference/mcp/). The kinds a card reads by are `decision`, `finding`, `pitfall`, `plan` (a step of the plan of edits), `change`, and `next` for the line the next session starts from.

## The packet of a card

```console
$ rigger task context WA-4130
# WA-4130 · rtf files are not supported

Status: active

## Worked in
- webapp (C:\work\webapp) on `fix/rtf` - where it is fixed

## Next step
Write the loader test first.

## Decisions
- 2026-09-08 · Fix the parser, not the export.

## Plan
- 2026-09-08 · Add the rtf branch to the loader.
```

What it is, where it is worked, the next step, and everything written against it by kind, newest first - within the same token budget as a project's [packet](/rigger/reference/context/), and saying what it left out when the budget bit. Over MCP the same is `task_context`, arriving with a line of instruction in front so the assistant treats it as the state of the work.

## Statuses

A task's status is a word from a vocabulary rather than one of two:

| Status | What it says |
| --- | --- |
| `new` | untouched - what a plan's empty box means |
| `active` | being worked on |
| `waiting-handoff` | handed to someone else; nothing to do until they answer |
| `frozen` | set aside on purpose |
| `done` | finished - what a ticked box means |
| `dropped` | struck from the hub; given by [`import`](/rigger/reference/import/), never by hand |

Everything before `done` is open work, and is counted as such by the [context packet](/rigger/reference/context/); the packet and the `plan` tool mark any word but `new` beside the task. `task status` takes a card's key or the id a project's packet lists; over MCP it is `set_task_status`, and `close_task` is `done` in one word. A hub's box does not overrule the word: reading a hub again leaves an active task active, and the export writes the box empty for anything short of `done`.

## Related

- [`mcp`](/rigger/reference/mcp/) - `task_find`, `task_context`, and the `task` argument of the recording tools.
- [`note`](/rigger/reference/note/) - the kinds, and the `plan` kind for cards.
- [`profile`](/rigger/reference/profile/) - the ticket profile a desk runs in.
