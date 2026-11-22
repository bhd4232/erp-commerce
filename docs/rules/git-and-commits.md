# Rule: Git, branches and commits

**Tier:** 1 for production branch and force pushes; 3 for commit style.

## Locks

- `main` moves only by a pull request the owner approves. Never push, merge or
  force-push to `main`.
- Never force-push, rebase, amend or reset a pushed branch. A rejected push
  means fetch, inspect and merge; an unclear conflict is a question for the
  owner.
- Never commit `.env`, secrets, tokens, credentials, build output or databases.
- Never skip, delete or weaken a test to make a run green.
- Never edit an applied migration; add the next `000N_*.sql`.

> **Owner action required:** GitHub `main` is currently **not protected**. Enable
> branch protection (pull request required, checks required, force-push and
> deletion blocked) and secret scanning with push protection. Until then this
> lock is only a written rule, not a machine lock.

## Branches and commits

- One topic per branch: `feature/<topic>`.
- One agent per branch at a time.
- Conventional Commits: `feat(auth): …`, `fix(order): …`, `docs(api): …`.
- One topic per commit; do not mix a refactor with a feature.
- A branch is brought up to date by merging `origin/main`, never by rebasing
  pushed commits.
- Files edited by every branch (`CHANGELOG.md`, `UPDATE_NOTES.md`,
  `CODEBASE_MAP.md`, docs) keep both sides' entries on conflict.

## Before every commit

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `./scripts/test.sh`
4. `./scripts/check_agent_rules.sh`
5. Update `CHANGELOG.md`, `UPDATE_NOTES.md`, the code map and the relevant
   `docs/` contract.
