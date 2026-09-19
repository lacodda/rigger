---
title: note
description: Record a decision, a finding, a pitfall, a change or the next step.
---

```
rigger note <PROJECT> <TEXT> [--kind <KIND>] [--principle <NAME>]
rigger wish <PROJECT> <TEXT> [--from <PROJECT>]
```

Records an event against a project. This is how the record grows during a session, instead of by editing markdown afterwards. An assistant does the same through the [MCP server](/rigger/reference/mcp/); this is the door for the owner and for scripts.

```console
$ rigger note sample "The parser must take hubs as they are." --kind finding
Recorded a finding for sample

$ rigger note sample "Ship the importer next." --kind next
Recorded a next for sample
```

## Kinds

| Kind | What it is for |
| --- | --- |
| `decision` | a decision and the reason behind it |
| `finding` | something learnt about the code or the domain |
| `pitfall` | a trap worth remembering |
| `change` | something that changed in the product |
| `next` | the one line the next session starts from |
| `state` | one line for the hub's state block: where things stand after this sitting |
| `plan` | a step of the plan of edits, for a [card](/rigger/reference/task/) |

An event against a [card](/rigger/reference/task/) rather than a project is `rigger task note <CARD> <TEXT> [--kind <KIND>]` - by the card's key, an alias or its id; cards live in the desk, so no project is named. A card's packet reads these back by kind.

`finding` is the default. Decisions, findings, pitfalls and changes appear under **Recent** in the [context packet](/rigger/reference/context/); the newest `next` becomes its **Next step** and is not repeated among the events.

`state` is not an event. It goes to the top of the README's «Состояние» block, which [`export`](/rigger/reference/export/) writes from the record - the one line the old ritual asked for whenever the state shifted, now with somewhere to go once the hub is generated:

```console
$ rigger note sample "**v0.2.0 shipped.** The second stage is closed." --kind state
Added a state line for sample; `rigger export` writes it into the README
```

## The principle a decision stands on

Most decisions of a line are not new: they are one more instance of something already believed. `--principle` names which, so the instances can be read together.

```console
$ rigger note sample "Dropped the old column outright." --kind decision \
      --principle "no users, no compatibility"
Recorded a decision for sample
  on the principle "no users, no compatibility"
```

[`rigger why --principle`](/rigger/reference/why/) then reads every decision that stands on it, across every project, oldest first - the story of how the line came to believe it, rather than six opinions in six hubs.

The vocabulary is whatever has been used. There is no list to keep: a principle has no properties beyond its name, and a list kept beside the decisions would be a second truth. `rigger why --principles` prints the names with how often each has been invoked, which is what to read before naming one - so a decision joins a thread rather than starting a synonym of it.

Only a decision can carry one. A finding or a change is something that happened, not something believed, and asking for a principle on one is refused rather than quietly dropped.

A question is not a kind here: questions are addressed to the owner, and they arrive from the hub or from an assistant calling [`ask_owner`](/rigger/reference/mcp/). Answering one is [`rigger resolve`](/rigger/reference/resolve/).

A `change` can also arrive on its own: [`rigger sync`](/rigger/reference/sync/) reads them from commit messages.

## `rigger wish`

A wish is something to sort into the plan later, so it needs no kind:

```console
$ rigger wish sample "Show how many days the project has gone without a commit."
Recorded a wish for sample
```

Wishes have their own section in the packet, and the same text recorded twice stays one wish - unlike a dated event, a wish is identified by what it says. Once a wish is in the plan, [`rigger resolve`](/rigger/reference/resolve/) takes it off the list.

### A wish a neighbour asked for

When one product needs something of another, `--from` records who is asking:

```console
$ rigger wish dowel "A status colour that is not the accent." --from lyrid
Recorded a wish for dowel from lyrid
```

It is still a wish - the same row, sorted by the same `resolve`, competing for the same stages. What changes is that somebody is waiting for it, so the packet names them on the line and [`inbox`](/rigger/reference/inbox/) gathers them into a group of their own, where the owner looks for what is waiting.

A name the record does not have is refused: a wish recorded from a neighbour that does not exist is one nobody will ever answer.

## Related

- [`context`](/rigger/reference/context/) - where these events are read back.
- [`mcp`](/rigger/reference/mcp/) - the same recording, as an assistant's tools.
- [`resolve`](/rigger/reference/resolve/) - closing a question or a wish.
- [`why`](/rigger/reference/why/) - reading a principle across the line.
- [`link`](/rigger/reference/link/) - tying two projects together.
