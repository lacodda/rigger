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
1 versions planned, 2 tasks open

## Current stage: v0.3.0 · Search
- full-text index
- a query language

## Waiting for the owner
- [2] Pick the release day.

## Recent
- 2026-09-03 · decision · The record is the database — Prose cannot be filtered.

## Next step
Ship the importer next.
```

## The budget

The packet has a token budget - 3000 by default, `--budget` to change it - and holds it. Everything a session cannot start without comes first and is never dropped: the state line, the current stage with its open tasks, the questions waiting for you, the wishes not yet sorted, and the next step. Recent events fill what is left, newest first, and the packet says how many older ones it left out:

```
(14 older events left out by the budget)
```

That line is the point of the budget. A packet that quietly ended its list would look like a project where nothing else ever happened.

Long events are summarised rather than truncated at a fixed width: the packet keeps the heading and the first sentence of the reasoning, then says how many characters remain. A decision in a real hub runs to fifteen hundred characters, and three of them at full length would crowd out a dozen others.

## `--explain`

Shows what each section costs, so an over-budget packet can be understood rather than guessed at:

```console
$ rigger context sample --explain
...
## Cost
state             32 tokens
current stage     18 tokens
questions          7 tokens
events            21 tokens
next step          6 tokens
total             96 tokens of 3000
```

Token counts are estimated from characters, not measured with a tokeniser: the number decides how much history to include, and an estimate that errs high is the safe direction.

## `--json`

The same packet as data, for a tool that renders it or an editor that feeds it to a model. Every field of the text form is there, plus `events_omitted`.

## Since the last sitting

When a [session](/rigger/reference/session/) has ended before, the state opens with a line saying where it stopped and what has happened since:

```
Last session ended yesterday: 3 events recorded, 7 changes committed
```

The question a returning assistant has is not "what has been going on" but "what changed while I was away" - and without this line the packet's recent events are undated history that every session re-reads from the top.

## Related

- [`find`](/rigger/reference/find/) - searching the events this packet samples from.
- [`why`](/rigger/reference/why/) - the whole story behind one version.

- [`note`](/rigger/reference/note/) - record what this session found, decided or is leaving for the next one.
- [`import`](/rigger/reference/import/) - fill the record from a hub before the first packet.
