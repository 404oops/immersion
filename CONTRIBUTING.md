# Contributing to Immersion

The core maintainer and repository owner is [@404oops](https://github.com/404oops). Contributions must be thorough, tested, verified, and checked for mistakes before submission. Contributors are responsible for every change they submit.

## Scope and breaking changes

Keep contributions focused and explain the problem, resulting behavior, and any limitations. Open an issue before proposing a breaking change and ping @404oops for a decision. Include compatibility impact, alternatives, and a migration plan. Wait for the maintainer's decision before submitting the breaking implementation. Changes to the `.musit` format, restore behavior, public interfaces, platform support, or established workflows may be breaking changes.

## Testing and review

Before submitting:

- Review the entire diff for mistakes, unrelated edits, sensitive data, and accidental generated files.
- Run `cargo fmt --all --check` and `cargo test --workspace --locked` for code changes.
- Build or check affected platforms; use `cargo build --workspace --locked` where supported. Follow [the development guide](wiki/Development.md) and [Linux build instructions](packaging/linux/README.md) for platform dependencies and packaging.
- Add meaningful regression coverage for changed behavior, including edge cases and failure paths. Verify filesystem, storage, and restore changes against disposable fixtures, including recovery and compatibility with existing data.
- Manually verify affected interface and platform integration behavior. Check documentation links and accuracy for documentation changes.
- Record the commands, results, platforms, manual checks, and any checks you could not run in the PR. Do not describe unrun checks as passing.

All required CI checks must pass before acceptance. Passing CI does not replace contributor review or platform verification.

## Ownership, licensing, and CLA

Contributed code is to be owned by the core maintainer through the copyright assignment in [CLA.md](CLA.md), also published as the [version 1 CLA gist](https://gist.github.com/404oops/39acdad2e557730c28e8d2fb54296f7b). Contributors must accept that agreement through CLA Assistant before their contributions can be accepted. Merely opening a PR or reading this file does not substitute for signing the agreement.

Accepted contributions are distributed under the repository's [license](LICENSE), currently GPL-3.0-only. Under the CLA, the owner may change the license for future distributions of rights they own or are authorized to relicense, at their discretion, without contacting contributors or obtaining their agreement. This does not revoke licenses already granted or transfer ownership of third-party material. Disclose any third-party material and its license before submitting it.

## Disclosure integrity

Complete the PR template's agent-use declaration accurately. Agent-assisted submissions must comply with [AGENTS.md](AGENTS.md), including the exact disclosure footer and commit trailer. Contributors must preserve required disclosure and any `biblioklept` record of an attempted concealment.

Deliberately removing required agent disclosure or the `biblioklept` marker is a violation: the PR is closed, and the contributor may be banned by the maintainer. A `disclosure-violation` label flags the PR for maintainer review; automated trace detection is evidence for review, not proof of intent. Honest mistakes should be disclosed and corrected promptly.
