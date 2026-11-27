# Contributing to Chariot

## Commit messages

Commits follow `type(scope): summary`, lowercase, imperative mood, no trailing period. Scope is optional and should name the crate you primarily set out to change, even if the commit also touches others.

Types:

- `feat`: real code changes, new functionality, dropped old functionality
- `minor`: real code changes, small tweak not worth calling a "feat"
- `fix`: bug fix
- `refactor`: internal restructuring, no behavior change
- `chore`: deps, licensing, CI, repo housekeeping. essentially anything that does not touch code

## Workflow

- One logical change per commit; attempt to keep the working tree buildable at each commit.
- Open a PR against `dev`. Rebase rather than merge to keep history linear.
- No `--no-verify` / force-push over shared branches.

## Before submitting

- `cargo build --locked` and `cargo fmt` must pass clean.
- `cargo clippy` warnings preferably fixed, not suppressed.
- `cargo deny check licenses` must pass if you touched dependencies.

## Licensing

Chariot is BSD-3-Clause. By contributing you agree your changes are licensed under the same terms. New third-party dependencies must have a compatible license (see the `accepted` list in `about.toml`, mirrored into the `allow` list in `deny.toml`; keep the two in sync) and `THIRD_PARTY_NOTICES.md` must be regenerated:

```
cargo install cargo-about
cargo about generate about.md.hbs -o THIRD_PARTY_NOTICES.md
```

Commit the regenerated file alongside the dependency change.

## AI use

AI-assisted contributions are allowed, but a human must review and understand everything before submitting it. You are responsible for AI-generated content exactly as if you wrote it yourself. "The AI did it" is not an excuse for bugs or license violations. Do not submit generated code you cannot explain. Disclosing AI use is appreciated, however, "assisted-by" signoff lines are optional.
