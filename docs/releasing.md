# Releasing a version

Unisson follows [Semantic Versioning](https://semver.org): `MAJOR.MINOR.PATCH`.

- **PATCH** (0.1.0 to 0.1.1): fixes only. Nothing the user sees works
  differently, except what was broken.
- **MINOR** (0.1.1 to 0.2.0): new features. Until 1.0.0, a minor version may
  also change how something works; the changelog says so.
- **MAJOR** (0.x to 1.0.0, then 1.x to 2.0.0): a change that breaks what
  existed, such as a settings file or a database that an older version cannot
  read.

A version that is not final gets a suffix, such as `0.2.0-rc.1`. Its release
is marked as a pre-release.

## Steps

1. On `develop`, set the new version in `Cargo.toml`, under
   `[workspace.package]`. Run `cargo check` so that `Cargo.lock` follows.
2. In `CHANGELOG.md`, rename `[Unreleased]` to the new version with the date
   of the day, add a new empty `[Unreleased]` above it, and update the links
   at the bottom.
3. Commit (`chore: release 0.2.0`), push, and wait for the CI to pass.
4. Merge `develop` into `main` and push.
5. Tag `main` and push the tag:

   ```sh
   git tag -a v0.2.0 -m "Unisson 0.2.0"
   git push origin v0.2.0
   ```

The tag starts the `Release` workflow. It checks that the tag matches the
version of `Cargo.toml` and that the changelog has a section for it, builds
the Debian package on Ubuntu 22.04 and the Windows installer, then creates the
release on GitHub with the notes of the changelog and a `SHA256SUMS` file.

The Windows installer is experimental: if its build fails, the release is
still published, without it.

## If something goes wrong

A release that should not have gone out can be deleted on GitHub, with its
tag. Fix the problem, then release a new patch version rather than moving a
tag that people may already have.
