# Contribution guidelines

Pull requests are welcome! If you wish to add a nontrivial feature, please
create an issue first so we can discuss the implementation. If adding a new
feature, please add test coverage as well.

## Git Hooks

We provide git hooks to enforce some lint rules. If you have
[cargo make](https://github.com/sagiegurari/cargo-make) installed, you can run
`cargo make install-hooks`. If not, you can copy the hooks from `.githooks` to
`.git/hooks`.

## Conventional commits

We use [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/)
for creating changelogs. These are checked using
[committed](https://github.com/crate-ci/committed). If you install the git
hooks, these are checked before commit.

You don't have to use the conventional style for your commits if you don't want
to, but please use it for your PR title. PRs are squash merged so the title will
be the commit message going into `main`.

## Formatting

We use some nightly-only features with rustfmt. If you're using rust analyzer,
you can enable running `rustfmt` with nightly. See
[here](https://github.com/rust-lang/rust-analyzer/issues/3627). If you're using
the command line, you can run `cargo +nightly fmt`. If you install the git
hooks, these are checked before commit.

## Tests

The test suite can be rather slow. It's recommended to use our test
configuration with `cargo make test`, which runs the tests with an increased
thread count, or run `cargo nextest run` and set the `NEXTEST_TEST_THREADS`
variable to something high (around 200 works well).

## Required Dev Packages

Tests have a transitive dependency on the `alsa-sys` crate. That crate needs the
`libasound2-dev` package installed on Debian based systems.
