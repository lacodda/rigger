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
