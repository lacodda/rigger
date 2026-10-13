---
title: project
description: Add, list and show projects.
---

A project is a repository rigger keeps a record of. This release records the repository itself; versions, tasks and events attach to it in the releases that follow.

## `project add`

```
rigger project add <PATH> [--name <NAME>]
```

Records the repository at `PATH`. The name is the directory name, because that is what a project is called in conversation - a manifest often declares something else, such as the crate `sample-cli` for the product `sample`, or a `*-workspace` root. Pass `--name` when the directory is not it either. The `origin` remote is read from the repository's git config when there is one.

```console
$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git
```

A path recorded before, or a name already taken, is refused with the existing project named in the message.

## `project list`

```
rigger project list [--json]
```

```console
$ rigger project list
sample  C:\dev\sample
```

A retired project is listed with the day it left: `attic  C:\dev\attic  (retired 2026-10-13)`. With `--json`, an array of the objects `project show --json` prints, the retired among them.

<!-- json: project list -->
| Field | Type | Meaning |
| --- | --- | --- |
| `[]` | object | One project per element, shaped as in `project show`. |
| `[].id` | integer | The project's id in the record. |
| `[].name` | string | The name the project is called by. |
| `[].path` | string | The repository's directory, or the name itself for a service project. |
| `[].remote` | string or null | The `origin` remote from git config; null when there is none. |
| `[].created_at` | string | When the project was recorded, in RFC 3339 UTC. |
| `[].tier` | string or null | The release tier: `A`, `B`, `C` or `out`; null until `project tier` sets one. |
| `[].rhythm_weeks` | integer or null | Weeks meant to go between releases; null until a tier or `--rhythm` sets it. |
| `[].kind` | string | `repo` for a repository, `service` for a place the record keeps for itself. |
| `[].hub_path` | string or null | The directory of the project's hub; null until a hub is imported or exported. |
| `[].gate` | string or null | The command that says the project is fit to commit; null when none is set. |
| `[].mark_code` | string or null | The two-lowercase-letter code of the mark, unique across the line; null when no mark is recorded. |
| `[].accent` | string or null | The colour the product owns, as `#RRGGBB`; null when not recorded. |
| `[].accent2` | string or null | The second colour, as `#RRGGBB`; null when not recorded. |
| `[].form` | string or null | What shape of thing the product is: `cli`, `desktop`, `web`, `library` or `service`; null when not recorded. |
| `[].docs_url` | string or null | Where the documentation lives; null when there is no site. |
| `[].on_session_end` | string or null | The command run when a sitting closes; null when none is set. |
| `[].retired_at` | string or null | When the project was retired, in RFC 3339 UTC; null while it is in the work. |
| `[].retired_by` | integer or null | The id of the decision that retired it, which `rigger event` reads; null while it is in the work. |


## `project show`

```
rigger project show <NAME> [--json]
```

```console
$ rigger project show sample
sample
  path:    C:\dev\sample
  remote:  https://github.com/acme/sample.git
  since:   2026-09-04T07:41:10Z

$ rigger project show sample --json
{
  "id": 1,
  "name": "sample",
  "path": "C:\\dev\\sample",
  "remote": "https://github.com/acme/sample.git",
  "created_at": "2026-09-04T07:41:10Z",
  "tier": "A",
  "rhythm_weeks": 2
}
```

Timestamps are UTC in RFC 3339. `tier` and `rhythm_weeks` are null until `project tier` sets them.

The commands that record or change a project - `add`, `service`, `set`, `mark`, `tier`, `retire` and `unretire` - print the same object under `--json`.

<!-- json: project add, project service, project show, project set, project mark, project tier, project retire, project unretire -->
| Field | Type | Meaning |
| --- | --- | --- |
| `id` | integer | The project's id in the record. |
| `name` | string | The name the project is called by. |
| `path` | string | The repository's directory, or the name itself for a service project. |
| `remote` | string or null | The `origin` remote from git config; null when there is none. |
| `created_at` | string | When the project was recorded, in RFC 3339 UTC. |
| `tier` | string or null | The release tier: `A`, `B`, `C` or `out`; null until `project tier` sets one. |
| `rhythm_weeks` | integer or null | Weeks meant to go between releases; null until a tier or `--rhythm` sets it. |
| `kind` | string | `repo` for a repository, `service` for a place the record keeps for itself. |
| `hub_path` | string or null | The directory of the project's hub; null until a hub is imported or exported. |
| `gate` | string or null | The command that says the project is fit to commit; null when none is set. |
| `mark_code` | string or null | The two-lowercase-letter code of the mark, unique across the line; null when no mark is recorded. |
| `accent` | string or null | The colour the product owns, as `#RRGGBB`; null when not recorded. |
| `accent2` | string or null | The second colour, as `#RRGGBB`; null when not recorded. |
| `form` | string or null | What shape of thing the product is: `cli`, `desktop`, `web`, `library` or `service`; null when not recorded. |
| `docs_url` | string or null | Where the documentation lives; null when there is no site. |
| `on_session_end` | string or null | The command run when a sitting closes; null when none is set. |
| `retired_at` | string or null | When the project was retired, in RFC 3339 UTC; null while it is in the work. |
| `retired_by` | integer or null | The id of the decision that retired it, which `rigger event` reads; null while it is in the work. |


## `project tier`

```
rigger project tier <NAME> <A|B|C|out> [--rhythm <WEEKS>]
rigger project tier --suggest [<NAME>] [--json]
```

Where a project sits in the release rotation, and how often it should ship. The tiers are the ones the line already works to: **A** carrying products that are released and used every day, **B** growing ones whose code works but whose circuit is not closed, **C** declared ones with a name and a plan and no product yet.

```console
$ rigger project tier sample A
sample is tier A - carrying - released, in the registries, used every day
  a release every 2 weeks
```

Each tier carries a rhythm of its own - two weeks for A, four for B, six for C - so setting one is a single word in the common case. `--rhythm` is for the project that keeps its tier's company but not its pace.

`out` is for a project deliberately outside the rotation: it is worked on when asked, and [`next`](/rigger/reference/next/) never says it is behind. That is a decision, not an omission - a project given `out` **and** a rhythm is still left alone.

A project with no tier at all is out by omission, and is equally left alone: rigger does not invent a schedule that nobody asked for.

### `--suggest`

```
rigger project tier --suggest [<NAME>] [--json]
```

A tier for each project, read from the releases of the last cycle (seven weeks, as [`retro --cycle`](/rigger/reference/retro/) counts them). It sets nothing: each line that would move prints the `project tier` command to paste.

```console
$ rigger project tier --suggest
From 2026-W33 to 2026-W39 (7 weeks) - nothing is set until you run the command

idle     stalled in B - 0 shipped, B asks 1
         rigger project tier idle C
racing   outgrew C - 12 shipped, C asks 1
         rigger project tier racing A
fresh    no tier yet - 2 shipped
         rigger project tier fresh B
steady   holds A - 3 shipped, A asks 3
aside    out by decision - 4 shipped
```

| Verdict | What the numbers say | Suggested |
| --- | --- | --- |
| `stalled` | the tier asked for releases and none came | one tier down; C goes `out` |
| `outgrown` | twice the tier's pace or more | the tier the pace describes; a racing A keeps A with a shorter `--rhythm` |
| `holds` | the tier describes what happened | the same tier |
| `out` | out of the rotation by decision | left out - the numbers are not held against it |
| `untiered` | no tier yet | the slowest tier whose rhythm the pace keeps; with nothing shipped, C if it has a plan and `out` if not |

A stall moves one step and a race as far as the pace goes, on purpose: stalling is a fact about the last weeks rather than a verdict on the product, and a product shipping forty times what its tier asks is not one step away from its tier. What would move is listed first, stalls before the rest. `--json` gives each project's `verdict`, `suggested`, `current`, `rhythm_weeks`, `shipped` and `expected`.

### JSON

With `--suggest`, `--json` prints the window and one suggestion per project.

<!-- json: project tier -->
| Field | Type | Meaning |
| --- | --- | --- |
| `from` | string | The first week of the window, as `YYYY-Www`. |
| `to` | string | The last week of the window, as `YYYY-Www`. |
| `weeks` | integer | How many weeks the window spans. |
| `suggestions` | array | One element per project, the ones that would move first. |
| `suggestions[]` | object | One project's suggestion. |
| `suggestions[].project` | string | The project's name. |
| `suggestions[].current` | string or null | The tier now: `A`, `B`, `C` or `out`; null when none is set. |
| `suggestions[].verdict` | string | What the numbers say: `stalled`, `outgrown`, `holds`, `out` or `untiered`. |
| `suggestions[].suggested` | string | The tier suggested: `A`, `B`, `C` or `out`. |
| `suggestions[].rhythm_weeks` | integer or null | A rhythm to go with the tier when its own would not fit; null otherwise. |
| `suggestions[].shipped` | integer | Versions shipped in the window. |
| `suggestions[].expected` | integer or null | Releases the current tier asks for over the window; null when it asks for none. |

## `project set`

```
rigger project set <NAME> [--gate <COMMAND>] [--no-gate]
                          [--on-session-end <COMMAND>] [--no-on-session-end]
```

Sets what the record keeps about a project beyond what git can tell it.

### `--gate`

The command that says a project is fit to commit: what CI runs, spelt for a shell.

```console
$ rigger project set sample --gate "cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test"
sample: gate is `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test`
Run it with: rigger gate sample
```

It lived in seventeen skill files before, one copy each, and drifted from the workflow it was supposed to mirror. Stated once, it is printed by the [context packet](/rigger/reference/context/) and run by [`rigger gate`](/rigger/reference/gate/).

With neither flag, `set` says what the gate is rather than doing nothing quietly:

```console
$ rigger project set sample
sample: gate is `cargo test`
```

`--no-gate` takes it off. An empty `--gate` is refused: an empty gate is not a gate.

### `--on-session-end`

The command to run when a sitting on this project closes.

```console
$ rigger project set rhapsod --on-session-end "pnpm publish:novellas"
rhapsod: a closing session runs `pnpm publish:novellas`
It runs after the sitting is written down, and its outcome is recorded.
```

Some projects end a session by doing something: publishing what was written, running a linter over it, pushing a stand. That belonged to a list in a skill file which the assistant had to remember at exactly the moment it was running out of context - which is when it is least likely to remember anything. Stated here, it happens.

It runs after the session is written down, because what it does belongs to the sitting that has just been recorded. The outcome is recorded as a change, with the command and how long it took.

A red result does not fail the close. The session is already over and already written down; refusing to end it would leave a sitting open for ever because a publish step could not reach the network. So it is said out loud and listed among what the ritual still asks for - visible, not fatal:

```console
$ rigger session end rhapsod
Session on rhapsod closed, open since 2026-09-19T08:12:04Z.
...
pnpm publish:novellas: red (exit 1) in 4 seconds

before you stop:
  `pnpm publish:novellas` came back red (exit 1) in 4 seconds
```

`--no-on-session-end` takes it off. It is a field of the project rather than a setting of the profile: what has to happen when a sitting on one project ends has nothing to do with the others.

## `rigger project mark <name>`

```
rigger project mark <NAME> [--code <XX>] [--accent <#RRGGBB>] [--accent2 <#RRGGBB>]
                           [--form <FORM>] [--docs <URL>] [--clear]
```

How a product looks from outside: the two-letter code of its mark, the colour it owns, what shape of thing it is, and where its documentation lives.

```console
$ rigger project mark sample --code sa --accent "#3FA873" --form cli --docs https://example.github.io/sample/
sample: mark recorded.
  code     sa
  accent   #3FA873
  form     cli
  docs     https://example.github.io/sample/
```

This existed in three places and agreed in none: a table in a notes vault, a constant in the project generator, and whatever a README happened to say. A hardcoded list does not know it is stale. Recorded here, it is published by [`export --line`](/rigger/reference/export/) and read by everything that needs it.

The forms are `cli`, `desktop`, `web`, `library` and `service` - a closed list, because a generator switches on it and a form spelt two ways is a branch that silently does not run. A code is two lowercase letters and unique across the line, which the schema holds rather than the habit. A colour is `#RRGGBB`.

What is not named keeps what it had, so `--form cli` states the form and not the whole mark; `--clear` takes the mark off. With no flags, `mark` says what is recorded.

## `project retire`

```
rigger project retire <NAME> --why <TEXT>
rigger project unretire <NAME> [--why <TEXT>]
```

Takes a project out of the work for good, keeping everything it has done readable.

```console
$ rigger project retire attic --why "Folded into sample; nobody climbs up there any more."
attic is retired, as of 2026-10-13: Folded into sample; nobody climbs up there any more.
  left as they were: 1 question, 1 wish, 1 open task - kept in its history, gone from the inbox and the plan
  gone from the line's skill, the registry, the calendar, the rotation, the inbox and sync
  `rigger show attic` still reads it; nothing new is recorded against it
  `rigger project unretire attic` brings it back
  C:\Users\you\.claude\skills\line\SKILL.md now lists 1 project
```

The tier `out` is not this. `out` is a project worked on when asked: the line's skill goes on offering it, the calendar goes on giving it a row, the inbox goes on asking its questions. A retired project is one nobody will work on, and every list that looks ahead leaves it out:

| Leaves it out | Still reads it by name |
| --- | --- |
| the line's [skill](/rigger/reference/skill/) - an installed one is written again on the spot | [`show`](/rigger/reference/show/), which says so in place of the tier, and the [context packet](/rigger/reference/context/), which opens with the day it was retired and the decision that says why |
| the registry [`export --line`](/rigger/reference/export/) publishes | [`event`](/rigger/reference/event/), [`why`](/rigger/reference/why/), [`find`](/rigger/reference/find/) and [`export`](/rigger/reference/export/) |
| [`calendar`](/rigger/reference/calendar/) and its `--ics`, [`next`](/rigger/reference/next/), [`week`](/rigger/reference/week/), [`release-day`](/rigger/reference/release-day/) | [`retro`](/rigger/reference/retro/) - what it shipped happened, and what it planned before it left was planned; a version aimed at a week after it left is not counted missed, and no tier is held against it |
| the tier signals, `project tier --suggest`, and the pairs [`link`](/rigger/reference/link/) warns have parted | `link list <name>` |
| [`inbox`](/rigger/reference/inbox/) - its questions wait for nobody now - and [`digest`](/rigger/reference/digest/) | `digest` with its name; its questions and wishes in its own packet |
| [`sync`](/rigger/reference/sync/), [`adopt`](/rigger/reference/adopt/) and `doctor --hubs` | `project list`, which marks it `(retired YYYY-MM-DD)`; [`doctor`](/rigger/reference/doctor/) counts it |
| the resources the [MCP server](/rigger/reference/mcp/) lists | every MCP tool that reads |

`--why` is required: a retirement is a decision, and it is recorded as one against the project before the door closes. What the project leaves open - questions, wishes, tasks - is counted and left as it was, part of its history rather than tidied away on its behalf.

Nothing new is recorded against it afterwards. `note`, `wish`, `session start`, `open`, `gate`, `version add` and `plan`, `project tier`, `link add`, `doc add`, `skill`, `sync` and `import` refuse it by name; so do the MCP tools that write. Below them the database refuses on its own: new events, sittings, stages and tasks of a retired project are rejected by the schema, whatever writes them. What it already holds can still be edited - a wish sorted with [`resolve`](/rigger/reference/resolve/), a task closed - so that it can be tidied; an answer to one of its questions is a new decision, and is refused.

```console
$ rigger note attic --kind decision "Paint it after all"
error: attic is retired since 2026-10-13: its history reads, but nothing new is recorded against it; `rigger project unretire attic` brings it back
```

A sitting still open on the project is refused first - close it with `session end`. A place the record keeps for itself (`project service`) is not in the work and cannot be retired. Retiring a retired project changes nothing and says so.

`unretire` brings it back, with the tier and rhythm it had. `--why` is optional there; the return is recorded as a decision either way.

```console
$ rigger project unretire attic --why "The ladder arrived"
attic is back in the work
  its tier is A, as it was; `rigger project tier` moves it
```

Under `--json`, both print the project as `project show` does.

## `rigger project service <name>`

Records a place the record keeps for itself: a project with no repository, and none coming.

```console
$ rigger project service line
Recorded 'line' as a place the record keeps for itself
  no repository: sync will not ask git about it
```

There is one thing this exists for. A [`retro`](/rigger/reference/retro/) is about every project at once, so its summary belongs to none of them - filing it under one would make it findable from the wrong place and invisible from the rest.

Such a place is not a repository that happens to be missing, and saying so matters: recorded as an ordinary project it made [`sync`](/rigger/reference/sync/) warn on every run that there was no git to read, and [`doctor`](/rigger/reference/doctor/) list it for ever among the projects waiting to be synced - the record nagging about a project working exactly as intended. Now `sync` does not ask git about it, and `doctor` does not count it as unsynced.

Everything else works on it as usual: events, questions, wishes, [`digest`](/rigger/reference/digest/), [`find`](/rigger/reference/find/) and the [context packet](/rigger/reference/context/).

## Related

- [`init`](/rigger/reference/init/) - the database `project` writes to.
- [`doctor`](/rigger/reference/doctor/) - how many projects are recorded.
- [`calendar`](/rigger/reference/calendar/) - where a tier and its rhythm are read.
- [`retro`](/rigger/reference/retro/) - what a service project is made for, and what it keeps of a retired one.
- [`gate`](/rigger/reference/gate/) - running the command `project set --gate` records.
