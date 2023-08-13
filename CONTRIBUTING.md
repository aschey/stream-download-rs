# Contribution guidelines

Pull requests are accepted. 
If you wish to add a nontrivial feature, please create an issue first so we can discuss the implementation before creating a pull request. 
Bugfix pull requests are welcome anytime.
If adding a new feature, please add test coverage as well.

## Conventional commits

We use [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/) and verify them on build.
These are checked using [committed](https://github.com/crate-ci/committed).
To check your commit messages as a git hook, you can run `cargo make install-hooks` or manually copy the hooks from `.githooks` to `.git/hooks`.