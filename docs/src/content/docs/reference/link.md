---
title: link
description: Tie two projects together, and see when a pair has parted company.
---

```
rigger link add <FROM> <TO> [--kind <KIND>] [--note <TEXT>]
rigger link list [PROJECT] [--json]
rigger link drift [--json]
rigger link remove <ID>
```

A line of products is not a list of products. Some of them are two halves of one thing, some draw on others, and until now all of that lived in prose - written once in each of two hubs, and true in neither after a month.

A link makes it a fact of the record, so it can be checked instead of read.

```console
$ rigger link add kasl@v1.13.0 kasl-server@v0.23.0 --note "the machine report"
[1] kasl v1.13.0 <-> kasl-server v0.23.0 (pair)
```

## The three kinds

| Kind | What it says |
| --- | --- |
| `pair` | two halves of one capability; neither ships alone |
| `consumer` | this project draws on the other |
| `donor` | the other project draws on this one |

`pair` is the default, and it is the only kind that is checked for drift. That is the whole reason the kinds are told apart: a consumer that has released four times since the thing it draws on last released is a consumer working normally, and a warning about it every week is a warning that stops being read.

`consumer` and `donor` are the same tie from its two ends, so a project can state whichever side it knows about. The row is recorded once and read from both:

```console
$ rigger link add lyrid dowel --kind consumer
[2] lyrid -> dowel (consumer)

$ rigger link list dowel
[2] donor     dowel -> lyrid
```

## Anchoring a version

A side is a project, or a project and a version written together:

```
kasl              the product, whatever version it is on
kasl@v1.13.0      that product at that version
```

The version is part of one argument rather than a positional of its own. With two optional versions between two projects, `rigger link kasl v1.13.0 kasl-server` has two readings, and a command line that guessed would anchor the wrong half - which is worse than not anchoring at all, because a link tied to the wrong version never reports the drift it was recorded to catch.

The number is matched by value rather than by text, the way [`why`](/rigger/reference/why/) already reads one: `v1.13`, `1.13` and `v1.13.0` all find the stage the record holds. The hubs of a line do not agree on how to spell a version - one product writes `v1.13` where its own other half writes `v0.23.0` - and a door that insisted on the text would make anchoring a pair a matter of remembering which of the two spells it which way. The confirmation prints the version as the record spells it, not as you typed it.

A version the project does not have is refused for the same reason.

A link with no version on either side is a tie between products rather than a promise about releases: it shows on both screens and is never checked for drift, because there is nothing to compare.

## Drift

```console
$ rigger link drift
kasl v1.13.0 shipped without kasl-server v0.23.0 — the machine report
alpha is 3 versions ahead of beta, which is still at v0.1.0
```

Two things are reported, and only for pairs:

**One half shipped alone.** One end has a tag and the other has not. The capability is out in the world with a piece missing, and nothing else in the record says so - this is the shape that went unnoticed twice on this line, both times found by a person reading two hubs side by side.

**One half ran ahead.** Neither end has shipped its half yet, but one has released two or more versions past where the pair was agreed while the other stood still. One version apart is the ordinary shape of the work - a pair is built one end at a time - so two is where the threshold sits.

You do not have to ask. [`next`](/rigger/reference/next/), [`week`](/rigger/reference/week/) and [`retro`](/rigger/reference/retro/) print the same lines under **Pairs out of step**, because a pair belongs to no single project and every screen that reads one project at a time is the reason the drift went unseen.

## Listing and forgetting

`rigger link list <PROJECT>` reads from that project's side - the near end is always the project you asked about, however the row was written. Without a project it lists every link in the record, once each.

```console
$ rigger link list kasl
[1] pair      kasl v1.13.0 <-> kasl-server v0.23.0  the machine report
```

`rigger link remove <ID>` forgets one, by the id the list prints.

Recording the same link twice changes nothing rather than adding a second row. A note added to a link that was recorded bare is kept, since the tie is the same tie.

## Related

- [`show`](/rigger/reference/show/) - the neighbours on a project's screen.
- [`week`](/rigger/reference/week/) - where the drift is raised without being asked for.
- [`note`](/rigger/reference/note/) - `--from`, for a wish one project asks another for.
- [`mcp`](/rigger/reference/mcp/) - `link` and `links`, as an assistant's tools.
