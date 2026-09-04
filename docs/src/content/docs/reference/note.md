---
title: note
description: Record a decision, a finding, a pitfall, a change or the next step.
---

```
rigger note <PROJECT> <TEXT> [--kind <KIND>]
rigger wish <PROJECT> <TEXT>
```

Records an event against a project. This is how the record grows during a session, instead of by editing markdown afterwards - and what the assistant's tools will call once the MCP server arrives.

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

`finding` is the default. Decisions, findings, pitfalls and changes appear under **Recent** in the [context packet](/rigger/reference/context/); the newest `next` becomes its **Next step** and is not repeated among the events.

A question is not a kind here: questions are addressed to the owner, and they arrive from the hub - or, once the MCP server lands, from the assistant asking one directly.

## `rigger wish`

A wish is something to sort into the plan later, so it needs no kind:

```console
$ rigger wish sample "Show how many days the project has gone without a commit."
Recorded a wish for sample
```

Wishes have their own section in the packet, and the same text recorded twice stays one wish - unlike a dated event, a wish is identified by what it says.

## Related

- [`context`](/rigger/reference/context/) - where these events are read back.
