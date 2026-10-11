---
title: The context packet
description: What the assistant reads to start a session, and why it has a budget.
---

A coding assistant that starts a session by reading the whole project record pays for every line of it, every time. Past a certain size the record no longer fits, and "read the hub" quietly becomes "read the tail and hope". The context packet replaces that.

`rigger context <project>` prints, in this order:

1. **State** - one line: current version, tier, last tag and its date, commits since.
2. **Current stage** - the version in progress and its open items.
3. **Waiting for the owner** - open questions, oldest first.
4. **Wishes, not yet sorted** - what is queued for the plan, the neighbours' orders among them.
5. **Recent events** - the last few decisions, findings, pitfalls and changes.
6. **Next step** - the one line the previous session left behind.

The packet has a **budget**, measured in an approximation of tokens and enforced by a test in the repository. When a project outgrows it, the packet leaves out the oldest of what it lists and says how much; it never truncates silently.

## The packet names, `event` reads

Every question, wish and event is one line with an id in brackets: a heading or a first sentence, and how much more there is. A wish written to be acted on cold runs to paragraphs, and forty of them printed whole once cost a packet twice its budget before a single event - the owner's queue had crowded out everything that had been decided. So the queue shares the budget with the events instead of standing in front of them, and a session reads whole only the line it picks up, with [`rigger event`](/rigger/reference/event/) or the MCP `event` tool.

## The packet is the contract

The same content is served by the MCP `context` tool, and the same lines read whole through its `event` tool, so a session started from the command line and one started from an editor see the same thing. Recording goes the other way through the same server: `record_decision`, `record_finding`, `record_pitfall`, `record_change`, `set_next_step`, `ask_owner`. The assistant never edits the record as text.

`--json` prints the packet as data for scripts and skills.
