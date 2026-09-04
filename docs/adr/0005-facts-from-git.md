# ADR 0005: Facts from git outrank recorded claims

- Status: accepted
- Date: 2026-09-04

## Context

"What is done" used to depend on whether the assistant wrote the journal. A stage could be marked closed in the plan while its tag was never pushed, and a tag could exist for a version the plan still listed as open. Neither reader noticed, because the plan was the only place they looked.

## Decision

Git is the source for shipping facts. A tag matching `v*` means the version shipped, on the tag's date. Commits since the last tag are activity; branches are the activity of tasks in the ticket profile. `rigger sync` reads all of it in-process through gix and never spawns `git`. Where the plan and git disagree, the record follows git and the disagreement is listed by `rigger doctor`; it is never silently corrected.

## Consequences

- Closing a stage in the plan without a tag is visible as a mismatch, not as progress.
- Dates on the calendar come from tags, so the calendar reflects what shipped rather than what was hoped.
- Reading in-process keeps `sync` fast enough to run at the start of every session; gix lives in 0.x majors, so its updates are handled deliberately at the start of a stage.
