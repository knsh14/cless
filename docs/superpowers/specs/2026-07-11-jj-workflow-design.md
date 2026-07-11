# jj workflow integration

Status: approved design.

## Goal

Standardize the daily jj workflow for this repo — solo development, trunk-only
(`main` bookmark + `v*` release tags, no feature branches) — and document it in
CLAUDE.md. Keep it simple: builtin jj commands only.

## Decisions

- **No jj aliases.** A `tug` alias (community convention for advancing the
  nearest bookmark) was considered and rejected: this repo only has `main`, so
  `jj bookmark set main -r @-` is explicit and short enough.
- **No config changes.** Repo config keeps only `trunk() = main@origin`; user
  config keeps only `[user]` identity.
- **CLAUDE.md is the single source of truth** for the workflow; its
  "Version control" section is rewritten around the standard loop below.

## Standard daily loop

```sh
jj new main                  # start work on top of main
# ...edit; jj auto-snapshots the working copy (no add/stage)...
jj st / jj diff              # inspect
jj describe -m "feat: …"     # set the commit message
jj new                       # finish; the completed change is now @-
jj bookmark set main -r @-   # advance main to the completed change
jj git push                  # push to origin
```

Why `jj new` before advancing main: commits reachable from `main@origin` are
immutable. If `main` is set to `@` and pushed, the working-copy commit itself
becomes immutable and the next edit errors out. Finishing with `jj new` keeps
`@` as a mutable scratch change on top. (The previous CLAUDE.md documented
`jj bookmark set main -r @`, which has this gotcha.)

Unchanged parts, kept in the doc:

- `jj git fetch` — local `main` auto-updates (`main@origin` is tracked)
- `jj undo` / `jj op log` — safety net
- Releases: `git tag vX.Y.Z && git push origin vX.Y.Z` (jj does not create tags)

## CLAUDE.md changes

Replace the command block in "Version control" with the loop above; keep the
intro paragraph (colocated layout, GitButler note) and the release-tag note.
