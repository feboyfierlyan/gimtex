# Contributing to Gimtex

Thanks for helping make code context easier to prepare and inspect.

[← README](README.md) · [Architecture](docs/architecture.md) · [Usage](docs/usage.md)

## Get a working checkout

Install Rust/Cargo and Git, then:

```sh
git clone https://github.com/feboyfierlyan/gimtex.git
cd gimtex
cargo build --locked
cargo run -- --help
```

Use a recent stable Rust toolchain. CI currently runs stable Rust on Linux,
macOS, and Windows; this repository does not declare a separate minimum supported
Rust version.

## Before changing behavior

Read the [selection rules](docs/usage.md#file-selection-rules),
[output contract](docs/output-format.md), and [architecture](docs/architecture.md).
For a bug, start with a small fixture that demonstrates the problem. For a new
option, explain the expected interaction with filters, Git diff, interactive
selection, and output destinations.

Keep changes focused. Avoid committing real credentials, private source exports,
build output, or machine-specific paths.

## Verify a change

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

If formatting needs updating, run `cargo fmt` and inspect the diff. When dependency
constraints change, update and commit `Cargo.lock` deliberately; CI uses `--locked`.

The integration tests create temporary files and local Git repositories. Git must
be on `PATH`. Tests simulate remote cloning rather than contacting a network
service, and they do not modify your clipboard. Platform-specific tests are gated
where necessary.

For changes to XML, validate with a parser as CI does:

```sh
cargo run --locked -- . -i '*.rs' -f xml -o context.xml
python3 -c 'import xml.etree.ElementTree as ET; ET.parse("context.xml")'
```

Use `python` if that is your platform's Python 3 command. Remove the generated
`context.xml` before committing. For clipboard, authentication, or picker changes,
record any relevant manual checks and the OS/session used.

## Keep screenshots honest

The README demo uses a checked-in fixture and actual binary output. Regenerate
it after changes that affect formatting, metrics, or CLI diagnostics:

```sh
cargo build --locked --release
python3 scripts/render_readme_demo.py
rsvg-convert -o assets/readme/terminal.png assets/readme/terminal.svg
```

Python 3 and `rsvg-convert` are documentation-maintenance tools, not runtime
requirements for Gimtex. `rsvg-convert` is supplied by librsvg. The script writes
the transcript and Markdown export into `docs/media/`; the command's absolute
scan path is shortened for readability. Review SVG/PNG layout and keep the image,
transcript, fixture, and documented metrics consistent.

The opening banner is a real PTY recording of Gimtex processing its own `src/`;
see [the GIF recording guide](docs/recording.md) to regenerate it. The architecture
diagram and alternative static hero are editable SVG files in `assets/readme/`.
Those SVGs are conceptual illustrations; the terminal images are recorded examples.
Do not turn a fixture's token count into an unsupported performance claim.

## Open a pull request

Describe the user-visible problem and resulting behavior. Include a concrete
before/after example when useful, the verification you ran, and any limitations.
Update docs when option semantics or output changes. The PR template provides a
short structure; remove sections that do not apply.

For bug reports, include the command, expected behavior, actual behavior, OS,
Gimtex version, and a minimal sanitized fixture. For feature ideas, explain the
workflow that is difficult today before proposing an interface.
