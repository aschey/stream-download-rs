# Contribution guidelines

Pull requests are accepted.
If you wish to add a nontrivial feature, please create an issue first so we can discuss the implementation before creating a pull request.
Bugfix pull requests are welcome anytime.
If adding a new feature, please add test coverage as well.

## Git Hooks

We provide git hooks to enforce some lint rules.
If you have [cargo make](https://github.com/sagiegurari/cargo-make) installed, you can run `cargo make install-hooks` or manually copy the hooks from `.githooks` to `.git/hooks`.

## Conventional commits

We use [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/) and verify them on build.
These are checked using [committed](https://github.com/crate-ci/committed).
If you install the git hooks, these are checked before commit.

## Formatting

We use some nightly-only features with rustfmt. If you're using rust analyzer, you can enable running `rustfmt` with nightly.
See [here](https://github.com/rust-lang/rust-analyzer/issues/3627).
If you're using the command line, you can run `cargo +nightly fmt`.
If you install the git hooks, these are checked before commit.

## Required Dev Packages

Tests have a transient dependency on the `alsa-sys` crate. That crate needs the `libasound2-dev` package installed on Debian based systems.
