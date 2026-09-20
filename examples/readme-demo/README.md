# README demo fixture

These two Rust files are the input for the recorded CLI example in the main
README. They contain no credentials and require no network access.

From the repository root:

```sh
cargo build --locked --release
target/release/gimtex examples/readme-demo/src -i '*.rs' -n -o context.md
```

On Windows, use `target/release/gimtex.exe`.

To regenerate the transcript and SVG image used in the README:

```sh
python3 scripts/render_readme_demo.py
```

The generator runs the built release binary, uses a temporary output directory,
and writes the actual exported payload and terminal transcript into `docs/media/`.
It also renders the transcript into `assets/readme/terminal.svg`. The absolute
scan path is shortened for display; content and metrics are unchanged.

To refresh the PNG displayed by GitHub, install `rsvg-convert` separately and run:

```sh
rsvg-convert -o assets/readme/terminal.png assets/readme/terminal.svg
```

The example's token and character counts apply only to this fixture. They are
not a performance benchmark, token-saving claim, or measurement of this repo.
