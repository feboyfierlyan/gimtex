# The live README banner

[← README](../README.md) · [Contributing](../CONTRIBUTING.md)

The opening GIF records **Gimtex exporting its own Rust implementation**, rather
than the small two-file fixture used farther down the README. It shows a real
interactive shell running:

```sh
gimtex src/ -i '*.rs' -n -o context.md
head -n 19 context.md
```

The first command selects Rust files, adds line numbers, writes `context.md`, and
reports payload metrics. The second opens the beginning of that actual file:
its tree, file headings, token counts, and numbered source.

## What is recorded

The recorder copies this checkout's `src/` without modification into a temporary
workspace and launches the built release binary from a real shell pseudo-terminal
(PTY). It records the terminal echo, stdout, and stderr, including the temporary
scan path. It does not substitute fabricated output or execute a mock Gimtex.

The recording contains a redaction notice because `scanner.rs` includes synthetic
secret-pattern test strings. That notice is real; it is not evidence of an actual
credential in this repository. Metrics describe the captured source revision and
settings, not a general performance or token-saving claim.

The GIF renders the captured stream using a green terminal theme. Typing and
reading pauses are intentional. It starts with a 1.2-second still of the completed
export so nonanimated viewers see a useful result, then replays the recording and
loops. A [static preview](../assets/readme/hero-poster.png) is also available.

## Regenerate it

Recording requires macOS, Linux, or WSL, Python 3 with Pillow, a monospace font,
and a built Gimtex release binary. These are documentation tools, not additional
requirements to use Gimtex.

```sh
cargo build --locked --release
python3 -m venv .venv-recording
.venv-recording/bin/python -m pip install Pillow
.venv-recording/bin/python scripts/record_readme_hero.py
```

The recorder finds Menlo on macOS or common DejaVu/Liberation Mono font paths on
Linux. To choose a different installed monospace font:

```sh
.venv-recording/bin/python scripts/record_readme_hero.py --font /path/to/Mono.ttf
```

On Windows, use WSL for this PTY-based recording. The application itself remains
supported by the existing Windows CI workflow.

Generated artifacts:

- [`assets/readme/hero.gif`](../assets/readme/hero.gif) — looping banner.
- [`assets/readme/hero-poster.png`](../assets/readme/hero-poster.png) — static completed-export frame.
- [`docs/media/hero.cast`](media/hero.cast) — timestamped output events in asciicast v2 format.
- [`docs/media/hero-transcript.txt`](media/hero-transcript.txt) — readable terminal transcript with normalized line endings and trailing whitespace.
- [`docs/media/hero-recording.json`](media/hero-recording.json) — commands, binary version, source hashes, payload hash, and timing notes.

Run the script from any working directory; it resolves the repository relative to
itself. Review the resulting frames and transcript before committing. Temporary
paths and capture timing vary between runs. Keep the GIF, poster, recording, and
metadata together when updating them.
