# Changelog

All notable changes to this project are documented in this file.

## [0.21.0] - 2026-09-19

### Bug Fixes
- Read a workspace's description from the crate that ships

### Documentation
- Make the readme a shopfront
- Use the line's license line
- Show the project's screen and the registration on install

### Features
- Write one skill for the line instead of one per project
- Search the documents as well as the events
- Name what the budget refused, and the documents there are
- Print the project's screen
- Publish the line as a registry the record owns
- Register the server and the hook while installing
- Say whether the MCP server answers

### Testing
- Follow find's two lists and the columns of schema 23

## [0.20.0] - 2026-09-16

### Bug Fixes
- Let a generated hub describe the record's versions, not strike them
- Refuse a hub that cannot be read instead of reading it as empty

### Features
- Keep the rituals in the record and print them with one command
- Let a project state its gate, and run it into the record
- Take the wishes file into the record and watch it afterwards
- Read a questionnaire's answers into the record

## [0.19.0] - 2026-09-15

### Bug Fixes
- Let a generated plan describe the record rather than overrule it
- Write a document back to the file it was read from

### Features
- Keep the last few copies of the record, and say how old the newest is
- Keep a project's handwritten texts in the record, not in files
- Start a document from the questions its kind exists to answer
- Read a hub's handwritten texts into the record
- Write the handwritten texts back out, and show what a check would lose
- Let an assistant read and write a project's handwritten texts

### Testing
- Give the editor stubs a script on disk instead of a quoted one-liner

## [0.18.0] - 2026-09-08

### Features
- Task cards - find, make, open, link, write against and close

## [0.17.0] - 2026-09-08

### Breaking Changes

- **Profiles with a record of their own, and a vocabulary of task statuses**
the database moves from `<data>/rigger.db` to
`<data>/profiles/line/rigger.db`. The first profile-aware rigger to run
moves it and keeps the old file as `rigger.db.before-profiles`; nothing
has to be done by hand, and `RIGGER_DATA_DIR` still names the root.
Schema 17 renames the task status `open` to `new`; a copy of the
database is taken before the migration, as always. `adopt` no longer
requires a root when the profile names one.

### Features
- Profiles with a record of their own, and a vocabulary of task statuses

## [0.16.0] - 2026-09-08

### Features
- Let a hub written from the record keep growing, and strike what a hand struck
- Withdraw a struck question, and keep a tag-only version out of the changelog

## [0.15.0] - 2026-09-08

### Bug Fixes
- Keep the day a tag says a version shipped
- Read a hub into a record that already holds it
- Name everything a hub gave, and skip the service project in the hub check
- Let a stale plan neither reopen a closed task nor shape a shipped stage
- Keep a hub path the way the platform spells it

### Documentation
- Point getting started at adopt

### Features
- Adopt a directory of repositories, and write a thin skill

### Testing
- Compare the hub path the way the record spells it

## [0.14.0] - 2026-09-06

### Bug Fixes
- Write the owner's queue in the shape the hub wrote it in
- Keep a stage's opening and closing prose on their own sides
- Put a block's stages back in the order they were written
- Keep the gap a diary left after the rule between two entries
- Let the order a hub had outrank the order a number implies
- Keep the blank lines a hub leaves around its rules
- Give a heading inside a stage to the stage it stands in
- Keep a diary in the order its sittings were written down
- Keep the blank lines a hub leaves where it writes no rule
- Let the hub check look where the hub actually is
- Read a README the way its hub wrote it
- Put back a column an edited migration left out

### Documentation
- The export command, and the hub check in doctor

### Features
- Say when a hub writes one version up twice
- Generate the README's state block from the record

## [0.13.0] - 2026-09-05

### Documentation
- Changelog for v0.13.0

### Features
- Make a sitting something the record knows

## [0.12.0] - 2026-09-05

### Documentation
- Changelog for v0.12.0

### Features
- Look back at a cycle and see what the plan was worth

## [0.11.0] - 2026-09-05

### Documentation
- Changelog for v0.11.0

### Features
- Give the owner a week to read

## [0.10.0] - 2026-09-04

### Documentation
- Changelog for v0.10.0

### Features
- Put the release calendar in the record

## [0.9.0] - 2026-09-04

### Documentation
- Changelog for v0.9.0

### Features
- Show the owner what waits and what moved

## [0.8.0] - 2026-09-04

### Documentation
- Changelog for v0.8.0

### Features
- Ask the record where and why

## [0.7.1] - 2026-09-04

### Documentation
- Changelog for v0.7.1

### Features
- Install rgr beside rigger

## [0.7.0] - 2026-09-04

### Documentation
- Changelog for v0.7.0

### Features
- Read changes out of commit messages

## [0.6.0] - 2026-09-04

### Documentation
- Changelog for v0.6.0

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
