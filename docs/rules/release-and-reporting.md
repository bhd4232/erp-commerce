# Rule: Release notes and hand-off report

**Tier:** 2 for the records; 3 for wording.

## Every task

- Add a user-facing entry to `CHANGELOG.md` under `Unreleased`
  (`Added`/`Changed`/`Fixed`/`Security`/`Technical Notes`), unless the task is
  only maintaining the changelog itself.
- Keep `UPDATE_NOTES.md` current: date, reason, important files, verification,
  what was not verified, commit status.
- Update `CODEBASE_MAP.md` and the module map when a module, route, job, table
  or command is added, moved or removed.
- Update `docs/API_SPEC.md` for endpoints and `docs/DATABASE.md` for schema in
  the same commit.
- Update `docs/decisions.md` when an owner decision changes a rule.

## Hand-off report (owner's language, this order)

1. **Result:** what now works, one or two sentences.
2. **Verification:** tests run and result; what was not verified and why.
3. **Size:** files and lines changed against the estimate.
4. **Owner actions:** keys, server commands, approvals.
5. **Open points:** decisions waiting for the owner; things noticed but left
   unchanged.
6. **Location:** branch and commit.

State a failed or skipped check as plainly as a passed one. Never hide a
failure, an unverified claim, or a decision you made.
