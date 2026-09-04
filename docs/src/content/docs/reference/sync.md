---
title: sync
description: Read tags and commits into facts - what shipped, and what has happened since.
---

```
rigger sync [<PROJECT>] [--json]
```

Reads a repository's tags and commits and writes what they prove into the record: a tag matching `v*` means that version shipped, on the date of the commit it points at, and the commits since the newest tag are the project's activity. Without a project name, every recorded project is read.

Git is read in-process through [gix](https://github.com/GitoxideLabs/gitoxide); `git` is never spawned.

```console
$ rigger sync sample
sample:
  shipped    v0.1.0 on 2026-09-04
  read       2 changes from commit messages
```

Running it again says so plainly. This command is meant to run at the start of a session, and a command that prints a screen every time is a command nobody reads:

```console
$ rigger sync sample
sample:
  nothing changed
```

In a run over every project, a project with nothing to report prints nothing at all - seventeen "nothing changed" lines would hide the one that matters.

## A tag outranks the plan

Where the plan and git disagree, the record follows git for the fact git can prove. A version with a tag is shipped, whatever the plan called it, and the date comes from the tag rather than from prose ([ADR 0005](https://github.com/lacodda/rigger/blob/main/docs/adr/0005-facts-from-git.md)).

The other direction cannot be proven the same way. A version the plan closed with no tag in git may be a stage marked done too early - or a tag that was simply never fetched. So it is reported and left exactly as written:

```console
$ rigger sync claimed
claimed:
  shipped    v0.1.0 on 2026-09-04
  no tag     v0.2.0 is closed in the plan
```

[`rigger doctor`](/rigger/reference/doctor/) lists these across every project, without reading git itself:

```console
$ rigger doctor
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
schema:    version 3
projects:  1
versions:  3
tasks:     1
sessions:  0
events:    0

closed in the plan, no tag in git (1):
  claimed      v0.2.0
  a tag would settle it; rigger does not change what you wrote
```

A project that has never been synced is named there too: before the first run, `doctor` cannot tell a claim from a fact, and says so instead of reporting a mismatch it has not checked for.

## What counts as a release

A tag is a release when it is `v` followed by a digit - `v0.5.0`, `v1.9`. Everything else in a repository's tags is left alone: `latest` and `nightly` move, and recording one as a version would put a phantom release in the record and on the calendar.

A release the plan never mentioned is reported as `(not in the plan)` - but only when it is above the oldest version the plan knows. Every hub starts somewhere, and a repository usually reaches further back than the notes about it; naming twenty releases from two years ago buries the patch that shipped yesterday without a stage.

Annotated and lightweight tags both work. The date is always the commit's, because a lightweight tag - what `git tag v0.5.0` makes - has no date of its own.

## Changes from commit messages

The line writes [Conventional Commits](https://www.conventionalcommits.org/), so a commit already says what kind of change it is. `sync` reads that and records it, which means the changelog side of the record stays current without anyone opening a session.

Only what changed the product is recorded: `feat`, `fix`, and anything marked breaking with `!` or a `BREAKING CHANGE:` footer. `chore`, `docs`, `test`, `refactor`, `style` and `ci` are how the work was done rather than what shipped - they are in git, which is where a question about them belongs. A message that does not follow the convention at all is not an error; it is simply not a fact this can read.

An event is dated by its commit, not by the sync that read it, so a project synced for the first time arrives with its history spread across the months it actually happened in.

The commit's hash is what makes recording it twice impossible - not its text, which can be amended between one sync and the next.

Changes reach the [context packet](/rigger/reference/context/) like any other event, but they are capped at a share of its budget. A commit can always be read again in git; a decision or a pitfall exists nowhere else, and a chronicle that filled the packet would cost a session the reasoning behind the code it is about to change.

## Activity

The commits since the newest tag, and when the last one landed. It answers a question the record cannot: a project can be busy in commits and silent in events, and "nobody wrote a note" is not the same as "nobody worked".

`sync` prints it alongside news; the [context packet](/rigger/reference/context/) carries it into every session:

```
1 commit since the last release, the last one today
```

## Schema

Schema version 3 adds the commit an event came from, and where a task sits in its stage. The database is copied aside before migrating - the copy is named for the schema it holds - and an older rigger refuses a database it does not understand rather than damaging it.

## Running it on a schedule

`sync` is a plain binary, so a scheduler runs it directly - no wrapper script, which matters where PowerShell's execution policy refuses one.

On Windows, once a day at nine:

```console
> schtasks /create /tn "rigger sync" /tr "%LOCALAPPDATA%\Programs\rigger\rigger.exe sync" /sc daily /st 09:00
SUCCESS: The scheduled task "rigger sync" has successfully been created.
```

On Linux or macOS, the same as a crontab line:

```
0 9 * * * ~/.local/bin/rigger sync
```

Running it when nothing has changed costs a walk of recent history and prints nothing, so a daily schedule is not something you have to think about again.

## Related

- [`doctor`](/rigger/reference/doctor/) - the mismatches across every project.
- [`context`](/rigger/reference/context/) - where activity reaches a session.
- [`import`](/rigger/reference/import/) - the plan that git is checked against.
