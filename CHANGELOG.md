# Changelog

All notable changes to this project are documented in this file.

## [0.6.0] - 2026-09-04

### Breaking Changes
- **Database schema version 2.** `sync` needs somewhere to record a project's activity and, on each version, where its shipping date came from - an imported hub carries dates written by hand, and they look exactly like a tag's. Migration is automatic on the first command after upgrading, and the database is copied aside first, named for the schema it holds (`rigger.v1-<stamp>.bak`); nothing has to be done by hand. An older rigger refuses a version 2 database rather than damaging it, so downgrading means restoring that copy.
- **A tag now outranks the plan.** A version with a tag is recorded as shipped on the date of the tag's commit, whatever an imported hub said about it - including its date. Where a hub claimed a release git cannot confirm, the claim is left alone and reported by `rigger doctor`.

### Features
- Read tags and commits into facts
## [0.5.0] - 2026-09-04

### Documentation
- Changelog for v0.5.0

### Features
- Serve the record to an assistant over MCP
## [0.4.2] - 2026-09-04

### Bug Fixes
- Name the newest release by its number, not by its row

### Documentation
- Changelog for v0.4.2
## [0.4.1] - 2026-09-04

### Bug Fixes
- Make the assistant launch work on every platform

### Documentation
- Changelog for v0.4.0
- Changelog for v0.4.1

### Features
- Start an assistant session with the packet in hand

### Testing
- Compare the assistant's directory by its tail
- Read past the byte-order mark PowerShell writes
## [0.4.0] - 2026-09-04

### Features
- Start a Claude Code session with the packet ready
## [0.3.0] - 2026-09-04

### CI
- Publish to the registries on the tag

### Documentation
- Changelog for v0.3.0

### Features
- Print the context packet a session starts from
## [0.2.0] - 2026-09-04

### Bug Fixes
- Keep the user PATH an expandable string on Windows

### Build
- Publish the wrapper as @lacodda/rigger

### Documentation
- Changelog for v0.2.0

### Features
- Import a notes hub into the record
## [0.1.0] - 2026-09-04

### CI
- Keep style commits out of the changelog

### Documentation
- Changelog for v0.1.0

### Features
- Adopt the lacodda line mark
- Record projects in a local database
