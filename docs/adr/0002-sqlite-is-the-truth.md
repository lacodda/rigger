# ADR 0002: SQLite is the record; markdown is an export

- Status: accepted
- Date: 2026-09-04

## Context

The record rigger replaces was kept as markdown: a plan, a changelog, a decision log, a journal, a wishlist per project. It has two readers, and it serves neither. The owner does not read it, because prose cannot be filtered or summed across projects. The assistant reads it whole at the start of every session, at full price, and the largest project's record no longer fits in a context window. A file can only be read entirely or by line ranges; an edit requires reading first and matching exact text, and an edit in the middle of a long journal is two expensive steps with a chance of breaking the format. A query returns exactly what was asked; a write is one call.

## Decision

The record lives in a local SQLite database, one file per profile. Every fact rigger shows is a query over it. The markdown hub the owner keeps in a notes vault is **generated** from the database by `rigger export` - byte-stable for an unchanged database - and is never the source. Long-form documents that are genuinely prose (vision, research notes) stay hand-written and are not imported.

## Consequences

- The assistant records through tools (MCP) and reads a packet; it never edits the record as text.
- Import from existing hubs is a one-time migration with a report; export is repeatable and idempotent, so the notes vault keeps a readable git history.
- Schema migrations are versioned inside the binary; `rigger doctor` reports the schema version.
- Two machines mean two databases until a sync layer exists; that is a later decision, not this one.
