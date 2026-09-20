# Using Gimtex

[← README](../README.md) · [Architecture](architecture.md) · [Output format](output-format.md)

## Choose an input

A target can be a directory, a single file, or a Git repository URL:

```sh
gimtex . -o context.md
gimtex src/main.rs -n -o main-context.md
gimtex https://github.com/rust-lang/rust-by-example -I -o remote-context.md
```

`https://`, `http://`, `ssh://`, `git://`, and `git@` prefixes are recognized as
remote targets when they do not refer to an existing local path. Git must be
installed for remote and diff modes. Authentication uses your existing Git
configuration; Gimtex does not manage GitHub credentials.

A remote is cloned with depth 1 into a temporary directory. Configuration is
loaded from that clone, and the temporary directory stays alive until extraction
finishes. An unchanged clone normally has no `--diff` results.

A bare `gimtex` invocation shows help. Any explicit option without a target
scans the current directory:

```sh
gimtex -i '*.rs' -o context.md
```

## CLI reference

Run `gimtex --help` for the installed version's complete reference.

- **`-i`, `--filter <GLOB>`** — Include paths matching a glob, relative to the selected directory. For a single-file target, paths are relative to its parent. Quote globs so your shell does not expand them first.
- **`-I`, `--interactive`** — Choose among discovered candidates. All start selected; Space toggles, arrows move, and Enter confirms. Selecting none leaves existing outputs untouched.
- **`-d`, `--diff`** — Include tracked working-tree and staged changes against `HEAD`, restricted to the selected path. Rename destinations are included. Deleted/untracked files are omitted. Before the first commit, staged additions are supported.
- **`-n`, `--numbers`** — Prefix source lines with line numbers. Source line endings and whether the final line ends in a newline are preserved in the numbered content; Markdown framing may add a newline before the closing fence.
- **`-f`, `--format <FORMAT>`** — Choose `markdown` (default) or `xml`. Other values fail validation.
- **`-o`, `--output <PATH>`** — Write the payload to a file instead of stdout. Its parent directory must exist. The destination is excluded from input.
- **`-c`, `--copy`** — Copy the payload to the OS clipboard. May be combined with `-o`; if either destination is requested, the payload is not printed to stdout.
- **`--max-size <BYTES>`** — Maximum size of each file, default `100000`. A file exactly at the limit is included. Zero permits only empty files.
- **`-h`, `--help`** and **`-V`, `--version`** — Print help or the installed version.

### Useful combinations

```sh
# Select changed Rust files interactively.
gimtex . --diff -i '*.rs' -I -n -o changes.md

# Scan one subtree from outside the repository.
gimtex /path/to/project/src --diff -o changes.md

# Export larger text files to XML.
gimtex . --max-size 500000 -f xml -o context.xml

# Save and copy the same payload.
gimtex src/ -o context.md -c
```

The snippets use POSIX-shell/PowerShell-style quoting. In Windows Command Prompt,
use double quotes around globs: `-i "*.rs"`.

## File selection rules

The effective input is the intersection of discovery rules, custom ignores, the
optional glob, the optional Git change list, and the interactive selection.

1. **Discovery:** `ignore::WalkBuilder` applies its standard filters. Hidden files are omitted during directory walks. `.gitignore` handling follows the library defaults and requires a Git repository; `.ignore` files are also supported.
2. **Generated directories:** Descendant directories named `node_modules`, `.git`, `target`, `dist`, `build`, `vendor`, and `.next` are pruned. An explicitly selected directory is still scanned, and regular files with these names are not excluded solely by name.
3. **Custom config:** `gimtex.toml` patterns can exclude files and directories. Negation can undo a previous custom rule but cannot override discovery exclusions. A file cannot be re-included while its parent directory is excluded.
4. **Glob and diff:** Globs match target-relative paths. The `glob` crate's default matching allows `*.rs` to match nested paths. Diff mode further restricts candidates to Git's changed tracked files.
5. **Selection:** Interactive mode presents the sorted candidates. Size, encoding, and redaction checks happen after the selection.
6. **Preparation:** Oversized files, any file containing a NUL byte, and invalid UTF-8 are skipped with diagnostics. Only the remaining files appear in the exported tree and count.

Discovered file and directory symlinks are not followed. A symlink explicitly
supplied as the target is resolved first. For explicitly selected files, do not
rely on directory-walk hidden-file behavior as a privacy boundary: inspect the
result before sharing it.

## Configuration

Put `gimtex.toml` at the root of the directory being scanned:

```toml
ignore = [
  "private/",
  "*.log",
  "!keep.log",
  "/generated.json",
]
```

Patterns use gitignore syntax. The leading slash anchors a pattern to the scan
root. Negation (`!`) only affects custom rules. If you want to exclude the config
file itself from output, add `gimtex.toml` to the ignore list.

The scanner does not search upward for a Gimtex configuration or merge the
caller's config with the target's. For example:

```sh
# Reads /project/gimtex.toml.
gimtex /project

# Reads /project/src/gimtex.toml, if present.
gimtex /project/src

# A single file uses its parent's configuration.
gimtex /project/src/main.rs
```

A missing config is fine. Invalid TOML, unknown keys, invalid pattern syntax,
and failures to read an existing config produce errors.

## Output destinations

Without `-o` or `-c`, stdout contains the payload; stderr contains progress,
redaction notices, skip reasons, and metrics.

Prefer Gimtex's own output flag when writing into a scanned directory:

```sh
gimtex . -o context.md
```

The destination is excluded from scanning, and Gimtex writes a temporary file
before replacing it. A single input file cannot also be its own output. An output
symlink is replaced as a link; its target is not overwritten. The output folder
must already exist.

Shell redirection, such as `gimtex . > context.md`, is managed by the shell, so
Gimtex cannot exclude that destination automatically. Write outside the scanned
tree if you need redirection.

When combining `-o` and `-c`, the file is written first. If copying then fails,
the command returns a nonzero status and the written file remains available.

## Troubleshooting

### `gimtex` is not found after installation

Verify Cargo's bin directory is on `PATH`, reopen your terminal, and run
`gimtex --version`. The default is `$HOME/.cargo/bin` on Unix or
`%USERPROFILE%\.cargo\bin` on Windows.

### A file is missing

Inspect the selection rules above and stderr. Common causes are hidden files,
ignore rules, an incorrect target-relative glob, the 100,000-byte default limit,
or a file that is untracked in diff mode. Increasing the size limit does not
make a binary or non-UTF-8 file eligible.

### `--diff` produces no files

The command compares tracked changes against `HEAD`. A clean working tree has
nothing to export. Untracked files do not count. Use a normal scan to include
eligible untracked files, or stage an intended addition yourself before using
`--diff`.

### A config seems to be ignored

Check the selected target directory. Scanning `src/` loads `src/gimtex.toml`, not
the repository root's Gimtex config. Standard Git ignore handling is separate
and follows the `ignore` crate's rules.

### The clipboard operation fails

Verify you are in a session with an available clipboard service. Headless/remote
sessions may not provide one. On Linux, persistence after exit depends on the
clipboard service. Use `-o context.md` when you need a file independent of that
service.

### Tokens differ from a model's reported count

Gimtex uses `cl100k_base`. Another tokenizer, message formatting, or additional
prompt instructions can change the count. Total payload tokens also include the
file tree and Markdown/XML framing, not just the source files.

### A command returns an error

Read stderr. Invalid arguments/config, unavailable targets, Git failures,
read/traversal errors, and output/clipboard failures return a nonzero status.
Expected file skips are reported and do not themselves fail the scan.
