# ADR 0003: One binary, profiles as configuration

- Status: accepted
- Date: 2026-09-04

## Context

The same person keeps two kinds of record: a product line at home, where the unit of work is a version closed by a tag, and tickets at work, where a task has an id and aliases, lives in branches and touches several repositories. The work machine forbids PowerShell scripts through the execution policy set by domain policy: `.ps1` helpers run only with a bypass flag that a stricter policy can override, and hooks that call them fail without a clear error. Two tools with a shared core would double every fix; a tool that needs scripts would not survive on the work machine.

## Decision

One binary. The differences between home and work are a **profile**: a configuration file naming the roots, the id pattern, the unit of work (tag or branch) and the incoming-material tray, plus its own database. No code path branches on "home" or "work"; commands read the active profile. Hooks and automation call the binary directly; nothing in the workflow is a script.

## Consequences

- A fix for one profile is a fix for both; the model is tested once.
- The remaining risk on a locked-down machine is an unsigned executable under application control or SmartScreen; that is checked with one run before the ticket profile ships, and answered with code signing if needed.
- Profile switching is explicit (`rigger profile use`), never guessed from the machine.
