---
title: resolve
description: Answer a question, sort a wish or clear a blocker, so it leaves the packet.
---

```
rigger resolve <PROJECT> <ID> [ANSWER]
```

Closes a question waiting for you, or a wish that has found its place in the plan. Both are recorded easily and, until this, could not be closed at all - so a wish you had already built went on appearing in every packet, suggesting work that was done.

Ids come from [`rigger inbox`](/rigger/reference/inbox/), or from the [context packet](/rigger/reference/context/), which lists them in brackets:

```
## Waiting for the owner
- [1] Pick the release day.

## Wishes, not yet sorted
- [4] Show how many days the project has gone without a commit.
```

A wish needs no answer - it is sorted, not settled:

```console
$ rigger resolve sample 4
Sorted [4]: Show how many days the project has gone without a commit.
```

A question takes one, and the answer becomes a decision - which is what an answer to a question about a project is:

```console
$ rigger resolve sample 1 "CSV first, ODS later."
Answered [1]: Pick the release day.
  the answer is recorded as a decision
```

The decision keeps the question above it, so the reasoning reads as a whole later.

A wish a neighbour placed can be more than sorted. `--built` says it is done, not only given a place in the plan, and once that is released the neighbour [moves up](/rigger/reference/queue/#moved-up) to take it:

```console
$ rigger resolve sample 6 --built
Built [6]: A date picker.
```

A [blocker](/rigger/reference/go/#when-something-out-of-reach-stops-the-tag) is cleared the same way, and the project is back in the queue:

```console
$ rigger resolve sample 9
Cleared [9]: The signing key has expired.
```

## Nothing is deleted

The record is a history: a question that was asked stays asked. Resolving changes its kind - to `answered`, `sorted`, `built` or `cleared` - so it leaves the packet's waiting lists while remaining in the record.

Only questions, wishes and blockers can be resolved. A decision, finding or pitfall is refused: closing one would quietly rewrite what happened.

```console
$ rigger resolve sample 7
error: [7] is a decision, not a question, a wish or a blocker; only those are resolved
```

## From an assistant

The MCP server offers the same as a `resolve` tool. An assistant should use it for a wish it has just implemented, and for a question **only when you have actually answered it** - the tool records your decision, not a guess at it.

## JSON

<!-- json: resolve -->
| Field | Type | Meaning |
| --- | --- | --- |
| `resolved` | object | The question, wish or blocker that was closed. |
| `resolved.id` | integer | Its id. |
| `resolved.project` | string | The name of its project. |
| `resolved.was` | string | What it was before: `question`, `wish` or `blocker`. |
| `resolved.body` | string | Its text. |
| `resolved.answer` | string or null | The answer recorded as a decision; null for a wish. |

## Related

- [`inbox`](/rigger/reference/inbox/) - the whole queue, across every project.
- [`note`](/rigger/reference/note/) - `rigger wish`, which records the other end.
- [`context`](/rigger/reference/context/) - where the ids come from.
- [`mcp`](/rigger/reference/mcp/) - the same, as an assistant's tool.
