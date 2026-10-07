---
title: show
description: The project's screen - what it is, where it stands, what it has written down.
---

```
rigger show <PROJECT> [--json]
```

Everything a person wants when they open a project, on one screen:

```console
$ rigger show sample
sample
A sample product

path       C:\dev\sample
remote     https://github.com/example/sample.git
hub        C:\notes\Projects\sample
tier       C · a version a week
gate       cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test

Last shipped v0.2.0 on 2026-09-10, 2 commits since
3 versions planned, 4 tasks open
1 waiting on you, 2 wishes unsorted

Current stage: v0.3.0 · The screen
    the screen itself
  > the documents on it
  x the commands table

Paired with:
  sample-server v0.3.0 (here: v0.3.0) — the machine report
Draws on:
  widgets
Neighbours are asking for:
  [88] A status colour that is not the accent — downstream

Written down:
  vision    Vision of sample                   2026-09-12  rigger doc show sample vision
  rituals   Rituals of sample                  2026-09-15  rigger doc show sample rituals

Next step
  Finish the screen, then the registry.
```

## `--json`

The screen as data: the same facts, without the text form's sentences.

<!-- json: show -->
| Field | Type | Meaning |
| --- | --- | --- |
| `name` | string | The project's name. |
| `about` | string or null | The one-line description, read from the repository's `Cargo.toml` or `package.json`. |
| `path` | string | The project's directory. |
| `remote` | string or null | The git remote's URL. |
| `tier` | string or null | The project's tier letter. |
| `rhythm_weeks` | integer or null | The tier's rhythm: how many weeks one version takes. |
| `gate` | string or null | The command that says the project is fit to commit. |
| `on_session_end` | string or null | The command the project runs when a session ends. |
| `hub_path` | string or null | The project's hub directory in the notes vault. |
| `last_shipped` | array or null | The newest shipped version, as a pair: the version, then the day it shipped (`YYYY-MM-DD`). |
| `last_shipped[]` | string | One element of that pair. |
| `delivery` | string or null | How far the newest version got past its tag: `tag-only`, `no-assets`, `released`, `publish-running`, `publish-failed` or `published`. |
| `versions_planned` | integer | How many versions are planned and unshipped. |
| `tasks_open` | integer | How many tasks are open. |
| `commits_since_tag` | integer or null | Commits since the newest tag, as the last `sync` read them. |
| `stage` | object or null | The current stage; `null` when there is none. |
| `stage.version` | string | The stage's version, like `v0.3.0`. |
| `stage.title` | string or null | The stage's title. |
| `stage.tasks` | array | The stage's open tasks that are not put down. |
| `stage.tasks[]` | object | One task. |
| `stage.tasks[].id` | integer | The task's id. |
| `stage.tasks[].status` | string | One of `new`, `active`, `waiting-handoff`, `frozen`, `done`, `dropped`. |
| `stage.tasks[].title` | string | The task's title. |
| `stage.asleep` | array | Open tasks asleep until a day. |
| `stage.asleep[]` | object | One sleeping task. |
| `stage.asleep[].id` | integer | The task's id. |
| `stage.asleep[].status` | string | The task's status. |
| `stage.asleep[].title` | string | The task's title. |
| `stage.asleep[].snoozed_until` | string | The day it wakes, `YYYY-MM-DD`. |
| `stage.set_aside` | array | Earlier versions whose open work is all frozen or asleep. |
| `stage.set_aside[]` | object | One version set aside. |
| `stage.set_aside[].version` | string | The version. |
| `stage.set_aside[].title` | string or null | The version's title. |
| `stage.set_aside[].frozen` | integer | How many of its open tasks are frozen. |
| `stage.set_aside[].asleep` | integer | How many of its open tasks are asleep. |
| `documents` | array | The handwritten texts the project has. |
| `documents[]` | object | One document. |
| `documents[].kind` | string | The document's kind, such as `vision` or `rituals`. |
| `documents[].slug` | string | The name `rigger doc show` takes. |
| `documents[].title` | string | The document's title. |
| `documents[].updated_at` | string | When it was last written, an RFC 3339 UTC moment. |
| `questions` | integer | How many questions wait for the owner. |
| `wishes` | integer | How many wishes are unsorted. |
| `links` | array | What the project is tied to, read from its own side. |
| `links[]` | object | One link. |
| `links[].id` | integer | The link's id. |
| `links[].kind` | string | `pair` (two halves of one capability), `consumer` (this project draws on the other) or `donor` (the other draws on this one). |
| `links[].near` | object | One end of the link: always the project being shown |
| `links[].near.project` | string | The project's name. |
| `links[].near.version` | string or null | The version anchored on this side, when one is. |
| `links[].near.shipped` | boolean or null | Whether that version has a tag; `null` when no version is anchored. |
| `links[].far` | object | One end of the link: the other project |
| `links[].far.project` | string | The project's name. |
| `links[].far.version` | string or null | The version anchored on this side, when one is. |
| `links[].far.shipped` | boolean or null | Whether that version has a tag; `null` when no version is anchored. |
| `links[].note` | string or null | The note recorded with the link. |
| `asked` | array | Open wishes a neighbour asked this project for. |
| `asked[]` | object | One wish. |
| `asked[].id` | integer | The wish's id. |
| `asked[].body` | string | The wish. |
| `asked[].asked_by` | string | The neighbour project waiting for it. |
| `next_step` | string or null | The line the last session left behind. |

## Not the context packet

[`context`](/rigger/reference/context/) is written for an assistant and spends its budget on recent events, because catching up on what happened is what a session has to do first. A person opening a project wants the opposite: what the thing is, what state it is in, what has been written down about it, and the line that says what is next.

So `show` has no budget and no event log. It replaces the hub's README as the surface a person reads - everything on that page is generated from the record anyway, and reading it meant opening a notes application to look at a file about the project you already had open.

## The columns

Only what the record knows is printed; a project with no tier has no tier line. The task marks are `x` done, `-` dropped, `>` active, `~` frozen or waiting, and a space for a task not started.

Each document carries the command that opens it, so that knowing a vision exists and reading it are one step apart rather than two.

## The neighbours

Who a project stands beside, what it draws on, and what draws on it - printed from its [links](/rigger/reference/link/) rather than from prose. This is the half of a README that used to be written by hand in two hubs and went stale in both.

The three kinds get three headings because they are three different facts: drawing on a product is not the same as standing beside it, and folding them into one list of neighbours would say it was.

Under them, the wishes a neighbour has asked this project for, each with the id that sorts it and the name of who is waiting. Those are the same wishes the [packet](/rigger/reference/context/) carries and the [inbox](/rigger/reference/inbox/) gathers; here they are on the screen of the project being asked.

A project tied to nothing prints no heading at all, rather than an empty one that reads as something gone missing.

## Related

- [`context`](/rigger/reference/context/) - the same record, written for an assistant.
- [`doc`](/rigger/reference/doc/) - the handwritten texts the screen lists.
- [`project`](/rigger/reference/project/) - what sets the tier, the gate and the mark.
