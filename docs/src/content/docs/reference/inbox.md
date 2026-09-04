---
title: inbox
description: Every question waiting for your answer, across all projects.
---

```
rigger inbox [--project <NAME>] [--json]
```

The queue of questions waiting on you, gathered from every project at once. Until this, they were never in one place - they sat in each project's notes, and a question could wait weeks because nobody was looking at that file.

```console
$ rigger inbox
6 questions in 3 projects

alpha        [  1] 2026-09-04  Place in the release calendar
             [  2] 2026-09-04  Sign the binaries?
beta         [  3] 2026-09-04  Place in the release calendar
             [  4] 2026-09-04  Sign the binaries?
gamma        [  5] 2026-09-04  Place in the release calendar
             [  6] 2026-09-04  Sign the binaries?

Asked by several projects - one answer settles each group:
  Place in the release calendar — alpha, beta, gamma
  Sign the binaries? — alpha, beta, gamma

Answer one with: rigger resolve <project> <id> "<answer>"
```

Grouped by project, because answering is done a project at a time, and oldest first within one - what has waited longest is what is most overdue.

## One answer, several projects

The block at the bottom is the reason this command groups rather than lists. Questions are copied between projects: a real record had three of them waiting on the same decision about a release calendar, and without the grouping it read as three separate jobs.

Grouping is by wording, not by meaning. rigger notices that two questions are spelt alike - case and punctuation aside - and does not guess at what they have in common. That is enough for the case it exists for: the same question written into several hubs.

A question only one project asks is not a group, and neither are two questions inside one project - the block is about an answer that settles several projects at once.

## Questions come from two places

A hub's "waiting for the owner" section becomes questions when it is [imported](/rigger/reference/import/), and an assistant adds one with the `ask_owner` tool over [MCP](/rigger/reference/mcp/). Both land in the same queue.

## Answering

[`rigger resolve`](/rigger/reference/resolve/) takes a question off the queue, and the answer becomes a decision in the record:

```console
$ rigger resolve alpha 1 "Tier B, reviewed each cycle."
Answered [1]: **Place in the release calendar:** no tier assigned yet.
  the answer is recorded as a decision
```

Nothing is deleted - the question stays in the record as what was asked, and only leaves the queue.

## Related

- [`resolve`](/rigger/reference/resolve/) - answering one.
- [`digest`](/rigger/reference/digest/) - the other direction: what moved.
- [`context`](/rigger/reference/context/) - the same questions, in a project's packet.
