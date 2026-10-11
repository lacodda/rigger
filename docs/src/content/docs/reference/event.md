---
title: event
description: Read one recorded event whole, by the id the packet printed.
---

```
rigger event <PROJECT> <ID> [--json]
```

Prints one event of the record whole - a wish, a question, a decision, a finding - by the id in brackets that the [context packet](/rigger/reference/context/) printed in front of it.

The packet names; this reads. A wish is written to be acted on cold, so it runs to paragraphs, and forty of them at full length cost a packet twice its budget before a single event. So the packet carries each question, wish and event as one line - a heading or a first sentence, with how much more there is - and the line's id is how a session gets the rest when it needs it:

```
## Wishes, not yet sorted
- [4] (charts asks) Read ODS too (+82 chars)
```

```console
$ rigger event sample 4
[4] wish · sample · 2026-10-07
asked by charts

**Read ODS too**

Some of the sheets arrive from LibreOffice, and the reader drops them today.
```

The first line says what it is, whose, and when it was recorded. Under it, what else the record knows about it, when it knows something: the card it was written against, the neighbour who asked for a wish, the day a question is due, the principle a decision stands on. Then the text, as it was written.

A question or a wish that was [resolved](/rigger/reference/resolve/) is still there to read; its kind says what happened to it - `answered` or `sorted`.

## Ids belong to one project

Ids are counted across the whole record, so a number copied from another project's packet would otherwise read as somebody else's text taken for this one's. It is refused instead, with the command that reads it:

```console
$ rigger event charts 4
error: [4] belongs to sample, not to charts; `rigger event sample 4` reads it
```

## From an assistant

The MCP server offers the same as an `event` tool, taking `project` and `id`. An assistant reads a wish whole before building it, and a question whole before working near it.

## JSON

The event in the form a write prints it - [`note`](/rigger/reference/note/) and [`wish`](/rigger/reference/note/) answer with the same object - so the two are a pair.

<!-- json: event -->
| Field | Type | Meaning |
| --- | --- | --- |
| `event` | object | The event, whole. |
| `event.id` | integer | Its id, as the packet prints it in brackets. |
| `event.project` | string | The name of its project. |
| `event.card` | string or null | The key of the card it was written against; null for a project. |
| `event.kind` | string | Its kind: `decision`, `finding`, `pitfall`, `change`, `question`, `wish`, `answered`, `sorted` and the rest. |
| `event.body` | string | The whole text. |
| `event.created_at` | string | The moment it was recorded, in RFC 3339 UTC. |
| `event.principle` | string or null | The principle a decision stands on. |
| `event.asked_by` | string or null | The project that asked for a wish. |
| `event.due` | string or null | The day a question is due, as `YYYY-MM-DD`. |

## Related

- [`context`](/rigger/reference/context/) - where the ids come from.
- [`find`](/rigger/reference/find/) - when there is no id yet, only a word.
- [`resolve`](/rigger/reference/resolve/) - closing a question or a wish once it is read.
- [`mcp`](/rigger/reference/mcp/) - the same, as an assistant's tool.
