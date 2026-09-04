---
title: resolve
description: Answer a question or sort a wish, so it leaves the packet.
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
$ rigger resolve sample 1 "Friday, as the calendar has it."
Answered [1]: Pick the release day.
  the answer is recorded as a decision
```

The decision keeps the question above it, so the reasoning reads as a whole later.

## Nothing is deleted

The record is a history: a question that was asked stays asked. Resolving changes its kind - to `answered` or `sorted` - so it leaves the packet's waiting lists while remaining in the record.

Only questions and wishes can be resolved. A decision, finding or pitfall is refused: closing one would quietly rewrite what happened.

```console
$ rigger resolve sample 7
error: [7] is a decision, not a question or a wish; only those are answered
```

## From an assistant

The MCP server offers the same as a `resolve` tool. An assistant should use it for a wish it has just implemented, and for a question **only when you have actually answered it** - the tool records your decision, not a guess at it.

## Related

- [`inbox`](/rigger/reference/inbox/) - the whole queue, across every project.
- [`note`](/rigger/reference/note/) - `rigger wish`, which records the other end.
- [`context`](/rigger/reference/context/) - where the ids come from.
- [`mcp`](/rigger/reference/mcp/) - the same, as an assistant's tool.
