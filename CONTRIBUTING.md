# Contributing to Aphyn

<!-- TODO: state whether external pull requests are accepted, and where to discuss larger changes first. -->

## Commit authorship

Every author, committer and `Co-Authored-By` email is checked against [`.github/allowed-commit-emails.txt`](.github/allowed-commit-emails.txt). Add a new contributor's address there in the same pull request that first carries their commits.

Commits and pull requests must not carry AI-assistant attribution: no `Co-Authored-By` trailer naming an assistant, no "Generated with [...]" line in the commit body or PR description. The person who ran the tool is the sole author of the work.

Both rules are enforced in CI by the *Commit Authorship* check ([`.github/workflows/commit_authorship.yml`](.github/workflows/commit_authorship.yml)). Run it locally before pushing:

```bash
bash .github/scripts/check-commit-authorship.sh origin/dev..HEAD
```

## Working on the docs

The documentation site lives in `docs/` and is built with Docusaurus.

```bash
cd docs
npm install
npm run start
```

GitHub Pages deployment is handled by [`.github/workflows/docs.yml`](.github/workflows/docs.yml).

## License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed under MIT OR Apache-2.0, without any additional terms or conditions.
