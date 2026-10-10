# Unisson

A unified audio player for Linux, written in Rust.
One search box and one play queue for internet radio, local files and YouTube.

The project is under construction. Current target: Ubuntu 22.04.

## Repository layout

- `crates/unisson-core`: the core library, with no user interface
  (common model, source contract, source registry).
- The audio engine and the desktop application will be added as separate crates.

## Requirements

- Rust 1.90 or newer (install it with [rustup](https://www.rust-lang.org/tools/install)).

## Useful commands

```sh
cargo fmt --all                                         # format the code
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo test --workspace                                  # run the tests
cargo doc --no-deps --workspace --open                  # read the API docs
```

## Branches

- `main`: stable state.
- `develop`: integration branch. Each task is done on a `feature/...` branch,
  then merged into `develop`.
- `develop` is merged into `main` when a coherent set of tasks is ready.

## Working method

1. For the language, rely on the official Rust documentation.
2. For every library, read its documentation before writing code.
3. If a problem resists, look at GitHub issues and forums.
4. One task at a time, tracked on the project Kanban board.

## Conventions

- Code, comments, documentation and commit messages are written in English.
- Public items are documented with rustdoc comments: a one-line summary,
  then details, then `# Errors`, `# Panics` and `# Examples` sections when relevant.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
