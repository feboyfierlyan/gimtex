# gimtex

**Turn a codebase into useful context.**

A Rust CLI for selecting source files and preparing Markdown or XML for LLM workflows. Includes interactive selection, Git-diff extraction, token counts, and pattern-based secret redaction.

![A recorded gimtex CLI example](assets/demo.svg)

The preview shows actual output from a two-file demo fixture. The 87-token payload describes that fixture only; it is not a speed benchmark. [Reproduce the example](https://github.com/feboyfierlyan/feboyfierlyan/blob/main/projects/gimtex.md).

## Quick start

Install [Rust and Cargo](https://www.rust-lang.org/tools/install), then:

```sh
git clone https://github.com/feboyfierlyan/gimtex.git
cd gimtex
cargo install --path .
gimtex --version
```

From the project you want to inspect:

```sh
# Export selected Rust files to Markdown.
gimtex src/ -i '*.rs' -o context.md

# Choose files interactively.
gimtex . -I

# Extract files identified by Git's diff.
gimtex . --diff -o changes.md

# Include line numbers or use XML output.
gimtex src/ -n -o context.md
gimtex src/ -f xml -o context.xml
```

`-i` filters paths with a glob; `-I` opens the interactive picker. Running `gimtex` without any arguments prints help. Supplying options without a path (for example, `gimtex -i '*.rs'`) scans the current directory. Unknown output formats are rejected.

## What it does

- Walks the source tree, applying ignore rules and a per-file size limit (100,000 bytes by default).
- Lets you narrow the input with glob filters, Git changes, or an interactive picker.
- Emits a file tree, source contents, and token counts using `cl100k_base`.
- Writes to stdout, a file (`-o`), or the clipboard (`-c`).
- Matches and redacts some common secret patterns.
- Can shallow-clone a supplied repository URL into a temporary directory before scanning it.

```sh
# Scan a public repository. Requires Git.
gimtex https://github.com/rust-lang/rust-by-example -I

# Copy output to the clipboard.
gimtex src/ -c

# Increase the per-file limit to 500,000 bytes.
gimtex src/ --max-size 500000
```

## How it works

[`src/main.rs`](src/main.rs) handles CLI arguments, optional configuration loading, and temporary clones. [`src/scanner.rs`](src/scanner.rs) handles traversal, selection, redaction, token counting, the file tree, and output.

The implementation uses `clap`, `ignore`, `glob`, `regex`, `tiktoken-rs`, `dialoguer`, and `arboard`. File paths are sorted before processing so the output order is deterministic.

## File selection and configuration

Paths in filters and exports are relative to the selected directory (or the parent
of a single input file). For example, `gimtex /path/to/project -i 'src/*.rs'` selects
Rust files below that project's `src/`. A filename glob such as `*.rs` also matches
nested files.

Put `gimtex.toml` in the selected directory, or alongside a single input file. A
remote scan uses the cloned repository's configuration. The current working
directory's configuration is not applied to a different target.

```toml
ignore = ["private/", "*.log", "!keep.log"]
```

Patterns use gitignore syntax, including negation and directory patterns. They
apply to normal and Git-diff scans. A negation only re-includes a file excluded by
a custom pattern; it does not override built-in or Git ignore rules. As with Git,
a file cannot be re-included while its parent directory is excluded. Invalid TOML,
unknown configuration keys, and invalid patterns cause an error.

Hidden files and ignored files are omitted. Git ignore rules follow the `ignore`
crate's defaults, including requiring a Git repository for `.gitignore` rules.
The subdirectories `node_modules`, `.git`, `target`, `dist`, `build`, `vendor`, and
`.next` are pruned. Discovered symlinks are not followed; an explicitly supplied
symlink target is resolved before scanning.

`--diff` runs in the selected repository and is restricted to the selected path.
It includes tracked working-tree and staged changes against `HEAD`, including
rename destinations. Deleted and untracked files are omitted. In a repository
without a first commit, staged additions are included. An unchanged remote clone
will normally have no Git diff.

## Export behavior

- Markdown uses fenced source blocks, with longer fences when the source contains
  backticks. XML is one `<codebase>` document with escaped attributes and text.
- Payloads contain no generated terminal color codes. Source text is retained
  after secret redaction and optional line numbering. XML-invalid control
  characters are replaced with `U+FFFD`.
- Files larger than `--max-size`, files containing NUL bytes, and non-UTF-8 files
  are skipped with a diagnostic. The tree and file count include only exported
  files. Other read/traversal failures return a nonzero status.
- Project summaries are derived only from included, sanitized root manifests;
  filtering out `Cargo.toml` or `package.json` also removes their summary.
- Per-file token counts describe sanitized, optionally numbered source text.
  Total tokens include the complete serialized payload. Literal tokenizer special
  tokens are counted as ordinary source text. Character counts count Unicode
  characters, not UTF-8 bytes.
- `-o` excludes that destination from scanning and replaces it only after the
  full payload is ready. Repeating an export cannot ingest its previous output.
  A single input file cannot also be the output. Replacing an output symlink
  replaces the link itself, not its target.
- `-o context.md -c` writes the file and copies the same payload. Clipboard
  failures return a nonzero status; an already written file remains available.
  Clipboard persistence on Linux depends on the active clipboard service.
- Diagnostics and metrics go to stderr; stdout contains only the payload.
  Interactive selection starts with all candidates selected; choosing none exits
  without writing or copying. Size and encoding checks still apply afterward.

## Current limits

- **Review before sharing.** Secret detection is pattern-based and will miss formats outside its rules. It does not certify that output is safe to publish.
- **Token counts depend on the tokenizer.** `cl100k_base` is an estimate for workflows using other tokenizers.
- **Performance depends on the input.** No comparative speed claim is made here; this repository does not currently include a benchmark suite.

## Development

```sh
cargo build --locked
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo run -- --help
```

The tests use local temporary fixtures and Git repositories; they do not contact
remote services or modify the system clipboard. CI runs on Linux, macOS, and
Windows and independently parses an XML export. Interactive terminal selection
and clipboard services also need environment-specific smoke testing.

[More projects by Boy](https://github.com/feboyfierlyan) · [Portfolio](https://feboyfierlyan.com/)
