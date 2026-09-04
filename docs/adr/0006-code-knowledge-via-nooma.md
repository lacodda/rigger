# ADR 0006: Code knowledge comes from nooma, not an index of our own

- Status: accepted
- Date: 2026-09-04

## Context

A recurring cost in assistant sessions is re-reading the same directories of a large codebase to answer questions the previous session already answered. The fix is an index of the repository - symbols, imports, module dependencies - tied to a commit hash, updated incrementally, with cached per-module summaries keyed by content hash. Tools that build such a graph on every call (by asking the model) are useful but not deterministic and not cacheable. A sibling product of the line, nooma, is an offline search over files and already plans an index of exactly this shape.

## Decision

rigger does not build a code index. Code knowledge comes from nooma used as a library; rigger's context packet gains a module summary section once that library exists (planned for 1.x). Until then the packet carries the record only, and says nothing about the code.

## Consequences

- One index for the line instead of two; nooma's roadmap moves "repository index by commit hash" ahead of vault search.
- rigger's 0.x releases have no code-reading dependency beyond git; the packet stays small.
- If nooma's index arrives later than rigger 1.0, rigger ships without it - the dependency is optional, not blocking.
