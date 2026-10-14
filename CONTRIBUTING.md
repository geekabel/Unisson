# Contributing to Unisson

Thank you for looking at Unisson. Bug reports, ideas, translations and code
are all welcome.

## Reporting a bug

Open an [issue](https://github.com/geekabel/Unisson/issues) with:

- the version of Unisson, and your system (for example Ubuntu 22.04, GNOME,
  and `echo $XDG_SESSION_TYPE`);
- what you did, what you expected, and what happened;
- for a message in the interface, the technical detail: leave the pointer on
  the message and a small box shows it.

For a security problem, follow [SECURITY.md](SECURITY.md) instead.

## Setting up

The [README](README.md#build-from-source) lists what to install.
[`docs/development.md`](docs/development.md) explains how to run each part,
and how the code is organized.

## Before you open a pull request

Run what the CI runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
node --test crates/unisson-app/ui-tests/*.test.mjs
```

If your change touches what only a real desktop can show (sound, windows, the
tray, the keyboard), run the matching steps of
[`docs/manual-tests.md`](docs/manual-tests.md) and say which ones in the pull
request.

## Conventions

- **Language.** Code, comments, documentation and commit messages are in
  English. The interface is in English and French: a new text needs both, in
  the files of `crates/unisson-app/ui/js/locales`. A test fails when one is
  missing.
- **Comments.** Explain why, not what. Every public item has a documentation
  comment. Each module starts with a few lines on its role, and on its design
  when it is not obvious.
- **Safety.** `unsafe` is forbidden in the workspace. Texts from outside are
  written with `textContent`, never as markup.
- **Dependencies.** Read the documentation of a library before using it, and
  prefer the standard library when it does the job.
- **Commits.** [Conventional Commits](https://www.conventionalcommits.org):
  `feat(app): ...`, `fix(radio): ...`, `docs: ...`. The body says why the
  change is needed.
- **Tests.** A fix comes with a test that fails without it, when a test is
  possible.

## Branches

- `main` holds what was released.
- `develop` is where changes meet. Open pull requests against `develop`.
- A release merges `develop` into `main` and tags it: see
  [`docs/releasing.md`](docs/releasing.md).

## Adding a language

1. Add the language to `Language` in `crates/unisson-app/src/texts.rs`, with
   its table of texts.
2. Add its table to each file of `crates/unisson-app/ui/js/locales`, and its
   code to `CATALOGS` in `locales/index.js`.
3. Add it to the language choice of the *Sources* screen
   (`ui/js/views/sources.js`).
4. Run the tests: they check that no text is missing.

## License

By contributing, you agree that your contribution is licensed under the MIT
license or the Apache License 2.0, at the choice of the user, like the rest of
the project.
