# Aphyn

## Overview

## Features

## Project Status

## Getting Started

## Requirements

## Build&Run engine

## Build game app

## Examples

## Documentation

The documentation site lives in `docs/` and is built with Docusaurus.

```bash
cd docs
npm install
npm run start
```

GitHub Pages deployment is handled by `.github/workflows/docs.yml`.

## Development Workflow

## Contributing

Commits and pull requests must not carry AI-assistant attribution (no `Co-Authored-By` trailer naming
an assistant, no "Generated with [...]" line in the commit body or PR description) — the person who
ran the tool is the sole author of the work. Every author, committer and `Co-Authored-By` email is
also checked against [.github/allowed-commit-emails.txt](.github/allowed-commit-emails.txt); add a
new contributor's address there in the same pull request that first carries their commits. Both rules
are enforced in CI by the *Commit Authorship* check
([.github/workflows/commit_authorship.yml](.github/workflows/commit_authorship.yml)), which you can
also run locally:

```bash
bash .github/scripts/check-commit-authorship.sh origin/dev..HEAD
```

## Roadmap

## License

Aphyn is dual-licensed under either of

- MIT License ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
