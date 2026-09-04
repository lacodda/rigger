---
title: The context packet
description: What the assistant reads to start a session, and why it has a budget.
---

A coding assistant that starts a session by reading the whole project record pays for every line of it, every time. Past a certain size the record no longer fits, and "read the hub" quietly becomes "read the tail and hope". The context packet replaces that.

`rigger context <project>` prints, in this order:

1. **State** - one line: current version, tier, last tag and its date, commits since.
2. **Current stage** - the version in progress and its open items.
3. **Waiting for the owner** - open questions, oldest first.
4. **Recent events** - the last few decisions, findings, pitfalls and changes.
5. **Next step** - the one line the previous session left behind.

The packet has a **budget**, measured in an approximation of tokens and enforced by a test in the repository. When a project outgrows it, the packet shortens the recent events and says so; it never truncates silently.

## The packet is the contract

The same content is served by the MCP `context` tool, so a session started from the command line and one started from an editor see the same thing. Recording goes the other way through the same server: `record_decision`, `record_finding`, `record_pitfall`, `record_change`, `set_next_step`, `ask_owner`. The assistant never edits the record as text.

`--json` prints the packet as data for scripts and skills.
