---
title: digest
description: What moved lately, five lines per project.
---

```
rigger digest [<PROJECT>] [--since <DAYS>] [--json]
```

What has happened lately, at five lines per project. Five is the constraint the command is built around: read across a line of projects, a paragraph each would be the notes all over again - which is what stopped being read in the first place.

```console
$ rigger digest
Since 2026-08-28

alpha
  shipped v0.1.0
  recorded 1 decision, 1 change
  waiting on you: 2 questions
  next: v0.2.0 · Search

Quiet: beta, gamma
```

The lines, in order: what shipped, what was recorded, what waits on you, what is next. A project has fewer lines when it has less to say - none of them is padding.

## Only what moved

Across the whole line, a project that has shipped nothing and recorded nothing is named in one line at the end rather than given a block of its own. Seventeen projects at five lines each is not a digest, and a block that says nothing happened says it at five times the length.

Silence is still reported - a project untouched for a fortnight is not the same as one that shipped yesterday - it just costs one line instead of five.

Naming a project shows it whether or not it moved:

```console
$ rigger digest alpha --since 30d
Since 2026-08-05

alpha
  shipped v0.1.0
  recorded 1 decision, 1 change
  waiting on you: 2 questions
  next: v0.2.0 · Search
```

## The window

`--since` takes days: `7d` (the default), `30d`, or a bare `30`. The window is printed rather than assumed, so a digest that looks quiet can be read as "quiet in these seven days" rather than "quiet".

Releases are counted by the day they shipped, which [`rigger sync`](/rigger/reference/sync/) reads from their tags. Events are counted by kind - the digest says how much was decided, not what; [`find`](/rigger/reference/find/) and [`why`](/rigger/reference/why/) answer the second question.

A stage name is shown without the asides a plan carries: `v0.13.1 · MCP *(deferred from v0.13.0)*` reads as `v0.13.1 · MCP` here. The aside belongs in the plan, not in five lines.

## Related

- [`inbox`](/rigger/reference/inbox/) - the questions this counts, in full.
- [`why`](/rigger/reference/why/) - what went into one of the releases named here.
- [`sync`](/rigger/reference/sync/) - what keeps the releases and changes current.
