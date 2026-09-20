# The output contract

[← README](../README.md) · [Usage](usage.md) · [Architecture](architecture.md)

Gimtex produces a single text payload. Source file paths are relative to the
selected directory, or a single input file's parent. The rendered tree has `.`
as its root and contains only files that survived selection and preparation.

Progress and diagnostics go to stderr; stdout is reserved for the payload when
neither `-o` nor `-c` is used.

## Markdown

Markdown is the default. It contains:

1. An optional **Project context** section, derived from included root manifests.
2. A **Project structure** section with a fenced tree.
3. A **File contents** section with a heading and fenced source for each file.

The heading identifies the relative path and the source's token count. Markdown
special characters in the heading are escaped; the file tree displays the path
without Markdown heading escapes. Source fences are longer than any consecutive
backtick run in the content, so embedded Markdown does not close the block early.

[Read the complete recorded example](media/demo-context.md). To generate it:

```sh
gimtex examples/readme-demo/src -i '*.rs' -n -o context.md
```

Source text has no generated ANSI styling. Markdown framing adds a newline
before a closing fence when needed; consumers requiring exact decoded source
boundaries should use XML.

## XML

```sh
gimtex examples/readme-demo/src -i '*.rs' -f xml -o context.xml
```

The document is UTF-8 XML with a single `<codebase>` root. Its structure is:

```text
codebase
├── project-context             optional human-readable manifest summary
├── structure                   rendered file tree as text
└── files
    └── file                    zero or more entries
        ├── @path               target-relative path
        ├── @tokens             decoded source token count
        └── text                sanitized, optionally numbered source
```

Attributes and text are escaped. Newlines, carriage returns, and tabs are
encoded as numeric character references so an XML parser preserves them,
including whitespace in path attributes. Characters forbidden by XML 1.0 are
replaced with `U+FFFD`. No extra leading or trailing newline is added inside a
`<file>` element.

Use an XML parser, not regex, to decode source text:

```python
import xml.etree.ElementTree as ET

root = ET.parse("context.xml").getroot()
for file in root.findall("./files/file"):
    path = file.attrib["path"]
    source_tokens = int(file.attrib["tokens"])
    source = file.text or ""
    print(path, source_tokens, len(source))
```

An empty selection from a noninteractive scan still produces a complete document
with an empty `<files>` element. Deselecting all candidates in interactive mode
instead exits before producing an export.

## Line numbers

With `-n`, each source line is prefixed by a right-aligned line number and ` | `:

```text
   1 | pub fn double(value: i32) -> i32 {
   2 |     value * 2
   3 | }
```

Numbers are applied after redaction. A multi-line private-key block can collapse
to one replacement marker, so later numbers describe the sanitized text and may
not match the original file's line numbers. Do not use an export as a source map.

## Token and character counts

**Per-file tokens** count the sanitized, optionally numbered source using
`cl100k_base`. XML counts are computed after XML-invalid character replacement,
before entity escaping. They do not include the heading, fences, or XML tags.

**Total tokens** count the exact serialized payload: tree, optional summary,
source blocks, paths, and all Markdown/XML framing. Total tokens are therefore
not simply the sum of per-file counts. Literal tokenizer special-token strings
inside source are treated as ordinary text.

**Characters** count Unicode scalar values in the final payload, not UTF-8 bytes
or user-perceived grapheme clusters. The file count includes only successfully
prepared files.

These metrics are written to stderr. They are not a model-provider billing
estimate or a guarantee that the same count applies to another tokenizer.

## Project context

When included by the selection, root-level `Cargo.toml` and `package.json` files
can contribute a project name and up to 15 direct dependencies per manifest.
Dependency values are simplified to their version string when available. This
is a brief summary, not a dependency graph, lockfile audit, or recursive
workspace analysis.

Summaries use sanitized content. Excluding a manifest with a glob, custom ignore,
interactive selection, or size limit also excludes its summary. Unparseable
manifest text can still be exported as source but will not generate a summary.

## Secret-pattern handling

The current rules cover selected generic key/password assignments, `sk-` key
patterns, AWS access-key ID patterns, and PEM-style private-key blocks. Matches
are replaced by markers such as `[REDACTED_SECRET]`, `[REDACTED_OPENAI_KEY]`,
`[REDACTED_AWS_KEY]`, and `[REDACTED_PRIVATE_KEY]`.

Detection is not exhaustive. Arbitrary credentials, secrets with unknown names
or formats, and sensitive information in filenames can remain. Ordinary values
can also match a rule. Inspect exports before sending or publishing them, and
use ignore rules to exclude sensitive files at the source.

## Consumers and compatibility

This is a human-oriented context format, not a versioned interchange schema.
Prefer XML parsing over scraping Markdown headings when integrating with other
tools, and pin the Gimtex version if your workflow depends on exact formatting.
File names that cannot be represented as UTF-8 are displayed lossily; the Linux
scanner still uses their original bytes to locate and read them.
