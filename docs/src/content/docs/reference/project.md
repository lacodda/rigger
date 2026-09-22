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

With `--json`, an array of the objects `project show --json` prints.

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
- [`retro`](/rigger/reference/retro/) - what a service project is made for.
- [`gate`](/rigger/reference/gate/) - running the command `project set --gate` records.
