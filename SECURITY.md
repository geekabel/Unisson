# Security policy

## Supported versions

Unisson is young: only the latest release gets fixes.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Use the private form of GitHub instead: on the
[Security tab](https://github.com/geekabel/Unisson/security) of the repository,
choose **Report a vulnerability**. Say what you found, how to reproduce it,
and which version you used.

You should get a first answer within a week. Once a fix is released, the
report is credited in the release notes, unless you prefer otherwise.

## What Unisson does to stay safe

- The interface is a local web page with a strict content security policy: no
  inline script, no remote script, and each window is only allowed the few
  Tauri permissions its page uses.
- Texts that come from outside (station names, video titles, file tags, titles
  on air) are always written as text, never as markup.
- External programs are started with a list of arguments, never through a
  shell, and what the user typed always comes after `--`.
- The radio directory is reached over HTTPS only, and only mirrors under its
  own domain are followed.
- Rust code is built with `unsafe` forbidden in the whole workspace.
- Dependencies are checked for known vulnerabilities every week by the
  `Audit` workflow.
- Releases are built by GitHub Actions from a tag, with actions pinned to a
  commit, and each release lists the SHA-256 sums of its files.

Unisson runs two programs it finds through `PATH`: `yt-dlp`, for YouTube, and
`gsettings`, to install the launcher shortcut on GNOME. Like any program, it
trusts the `PATH` of the session it runs in.
