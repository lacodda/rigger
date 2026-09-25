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
rigger task incoming [--all] [--json]
rigger task take <KEY> [--project <NAME>]... [--branch <BRANCH>]
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

## Tickets from kasl's inbox

At a desk the tracker is already being asked: [kasl](https://kasl.lacodda.com/) polls it for the issues assigned to the person and keeps them in its inbox - new, changed, taken, asleep, gone. Since v0.25.0 rigger reads that inbox instead of asking the tracker a second time, with a second set of credentials and a second answer to the same question.

```console
$ rigger task incoming
kasl's inbox: 14 tickets - 3 without a card, 1 gone with the card still open

Without a card:
  WA-4131  taken  In Progress    Export drops the footer
  WA-4200         Open           Import skips empty rows
  WA-4212         Open           The report header wraps
Take one with: rigger task take <KEY>

The ticket has gone - closed or reassigned - and the card is open:
  WA-3999  active           gone 2026-09-24  Old import bug
Close one with: rigger task close <KEY>
```

The tickets **without a card** are the ones present in the tracker that no card is named by - by its key or by an alias - in kasl's order, with the ones already taken in kasl first: the person has said those are theirs. A ticket put to sleep in kasl is not waiting and is not listed; the first ten are named and the rest counted, unless `--all`. The cards **whose ticket has gone** are the open ones kasl has seen leave the tracker: the work may be over, or somebody else's now.

`rigger task take <KEY>` makes a card from a ticket by its key alone - the title is the tracker's, so nobody types it twice - puts it in hand, and takes `--project` and `--branch` as [`task new`](#making-and-keeping-a-card) does. A ticket that already has a card is put in hand instead of being made twice:

```console
$ rigger task take WA-4131
Made card WA-4131 from kasl's inbox and opened it: Export drops the footer
  ticket: In Progress · priority High · score 8 · taken 2026-09-21
          https://tracker.example.com/browse/WA-4131
```

The ticket's facts are not copied into the card. They stay kasl's, and a card that is a ticket shows them on its [screen](#what-git-says-about-a-card) and in its [packet](#the-packet-of-a-card) as kasl says them at that moment - including that it has gone.

### The shape rigger reads from kasl

rigger runs `kasl inbox list --all --snoozed --json` - `--all` keeps the tickets that have gone from the tracker, `--snoozed` the sleeping ones - and gives it five seconds. The answer is one object:

```json
{ "issues": [{
  "key": "WA-4131", "summary": "Export drops the footer",
  "status": "In Progress", "priority": "High", "score": 8.0,
  "url": "https://tracker.example.com/browse/WA-4131",
  "first_seen": "2026-09-19T09:00:00", "taken_at": "2026-09-21T10:00:00",
  "pinned": false, "snoozed_until": null, "gone_at": null
}] }
```

| Field | What it holds |
| --- | --- |
| `issues` | the tickets, in the order kasl lists them |
| `key` | the ticket's id, as the tracker spells it |
| `summary` | its title - what `task take` names the card |
| `status` | the tracker's status, by name |
| `priority` | the tracker's priority, by name |
| `score` | the ranking field kasl sorts by, when the tracker has one |
| `url` | where the ticket is in the tracker |
| `first_seen` | when kasl first saw it |
| `taken_at` | when the person took it in kasl |
| `pinned` | whether it is pinned in kasl |
| `snoozed_until` | when a ticket put to sleep is due back |
| `gone_at` | when it stopped appearing in the tracker: closed, or given to somebody else |

Only `key` and `summary` are required; every other field may be missing or `null`, and a field rigger does not know is ignored, so kasl can say more without breaking the reader. Moments are kasl's local time, and rigger shows the day of them.

`task incoming` and `task take` say so when kasl cannot answer - none installed, or a kasl whose inbox has no `--json` yet - and fail. A card's screen and packet do not: a card is shown the same without its ticket. A scratch record reaches no kasl unless `RIGGER_KASL` names one. A machine without kasl makes cards with [`task new`](#making-and-keeping-a-card); a tracker plugin for it is on the way to 1.x.

`task incoming --json` is every ticket kasl gave, in its order, each with the fields above and a `card` - its `key`, `title` and `status`, or `null` when no card is named by the ticket - under `source`, which is `kasl`:

```json
{ "source": "kasl", "issues": [{ "key": "WA-4131", "summary": "...", "card": null }] }
```

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
  tray:     C:\...\trays\WA-4130 - the form filled, 2 files waiting
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
  "events": 0,
  "ticket": { "key": "WA-4130", "summary": "...", "status": "In Progress", "priority": "High", "score": null, "url": "...", "first_seen": "...", "taken_at": null, "pinned": false, "snoozed_until": null, "gone_at": null },
  "tray": {
    "path": "C:\\...\\trays\\WA-4130",
    "form": { "path": "C:\\...\\trays\\WA-4130\\incoming.md", "filled": true },
    "files": [{ "path": "shot.png", "bytes": 217088, "modified": "2026-09-25T17:22:29Z" }],
    "sorted": [{ "day": "2026-09-20", "files": 5 }]
  }
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
| `ticket` | the ticket in kasl's inbox the card is, by its key or an alias, in [the shape rigger reads](#the-shape-rigger-reads-from-kasl); `null` when kasl is not there or does not know it |
| `tray` | the card's [tray](/rigger/reference/tray/), `null` when it has none: its `path`; the `form` - its `path` and whether it is `filled`; the `files` waiting, each with its `path` in the tray, `bytes` and `modified` (UTC); and the rounds `sorted` before, each a `day` and how many `files` |

### What waits in the tray

A card with a [tray](/rigger/reference/tray/) says what waits in it: on its screen in one line - whether the form is filled, how many files wait, the last round sorted - and in its packet under `## Tray`.

## Writing against a card

Everything a session learns goes against the card as events: `rigger task note <CARD> <TEXT> --kind <KIND>`, or the `task` argument of the recording tools over [MCP](/rigger/reference/mcp/). The kinds a card reads by are `decision`, `finding`, `pitfall`, `plan` (a step of the plan of edits), `change`, and `next` for the line the next session starts from.

## The packet of a card

```console
$ rigger task context WA-4130
# WA-4130 · rtf files are not supported

Status: active

## Ticket
In Progress · priority High · taken 2026-09-21
https://tracker.example.com/browse/WA-4130

## Worked in
- webapp (C:\work\webapp) on `fix/rtf` - where it is fixed

## Tray
C:\...\trays\WA-4130
- the form is filled; read it first: C:\...\trays\WA-4130\incoming.md
- 2 files waiting: shot.png (212 KB), logs/app.log (48 KB)

## Next step
Write the loader test first.

## Decisions
- 2026-09-08 · Fix the parser, not the export.

## Plan
- 2026-09-08 · Add the rtf branch to the loader.
```

What it is, its ticket as kasl sees it, where it is worked, what git and its tray hold, the next step, and everything written against it by kind, newest first - within the same token budget as a project's [packet](/rigger/reference/context/), and saying what it left out when the budget bit. Over MCP the same is `task_context`, arriving with a line of instruction in front so the assistant treats it as the state of the work.

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
- [`tray`](/rigger/reference/tray/) - the material that arrives for a card.
