# ADR 0004: CLI and an MCP server before any window

- Status: accepted
- Date: 2026-09-04

## Context

rigger has two users. The owner wants three screens: what waits for a decision, the five-line digest, the release calendar. The assistant wants a context packet at the start of a session and a way to record events during it. The second user is the one whose cost is measured today, and the one whose ritual is the most expensive to keep by hand. A desktop window built first would delay the packet and the recording tools, which is where the value is.

## Decision

The first door is the command line and an MCP server speaking over stdio. `rigger context` and `rigger mcp` are the product until the MVP criterion is met: one real session on one real project runs without reading the hub. The desktop window comes after 1.0 and reuses the Tauri shell of a sibling product rather than growing its own.

## Consequences

- Every owner-facing view exists as a command and as JSON before it exists as a screen; the window renders what the CLI already computes.
- The MCP tool set is the assistant's only way to write; its names and shapes are the contract the window will call too.
- Skills that today read a hub become thin: they call `rigger context` and `rigger session end`.
