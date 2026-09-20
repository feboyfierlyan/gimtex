use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use glob::Pattern;
use ignore::{
    gitignore::{Gitignore, GitignoreBuilder},
    WalkBuilder,
};
use rayon::prelude::*;
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tiktoken_rs::cl100k_base;

struct SecretScanner {
    generic_keys: Regex,
    openai_keys: Regex,
    aws_keys: Regex,
    private_keys: Regex,
}

impl SecretScanner {
    fn new() -> Result<Self> {
        Ok(Self {
            // Replace only the value, preserving the key, separator, and quote style.
            generic_keys: Regex::new(
                r#"(?i)\b(?:api[_-]?key|auth[_-]?token|access[_-]?key|access[_-]?token|secret(?:[_-]?key)?|password|aws_secret_access_key)\b["']?[ \t]*[:=][ \t]*(?:"(?P<double>(?:\\[^\r\n]|[^"\\\r\n])*)"|'(?P<single>(?:\\[^\r\n]|[^'\\\r\n])*)'|(?P<bare>[^\s"'#,;]+))"#,
            )?,
            openai_keys: Regex::new(r"\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}")?,
            aws_keys: Regex::new(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")?,
            private_keys: Regex::new(
                r"(?s)-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----.*?-----END (?:[A-Z0-9]+ )*PRIVATE KEY-----",
            )?,
        })
    }

    fn scan(&self, content: &str, file_path: &Path) -> String {
        let sanitized = self
            .private_keys
            .replace_all(content, "[REDACTED_PRIVATE_KEY]");
        let sanitized = self
            .generic_keys
            .replace_all(&sanitized, |caps: &regex::Captures| {
                let whole = caps.get(0).expect("regex match");
                let value = caps
                    .name("double")
                    .or_else(|| caps.name("single"))
                    .or_else(|| caps.name("bare"))
                    .expect("secret value");
                if value.is_empty() {
                    return whole.as_str().to_owned();
                }
                let start = value.start() - whole.start();
                let end = value.end() - whole.start();
                format!(
                    "{}[REDACTED_SECRET]{}",
                    &whole.as_str()[..start],
                    &whole.as_str()[end..]
                )
            });
        let sanitized = self
            .openai_keys
            .replace_all(&sanitized, "[REDACTED_OPENAI_KEY]");
        let sanitized = self
            .aws_keys
            .replace_all(&sanitized, "[REDACTED_AWS_KEY]")
            .into_owned();
        if sanitized != content {
            eprintln!(
                "{} Potential secret redacted in: {}",
                "[!]".yellow().bold(),
                file_path.display()
            );
        }
        sanitized
    }
}

#[derive(Default)]
struct TreeNode {
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn insert(&mut self, path: &Path) {
        let mut current = self;
        for component in path {
            current = current
                .children
                .entry(component.to_string_lossy().into_owned())
                .or_default();
        }
    }

    fn render(&self, prefix: &str) -> String {
        let mut output = String::new();
        for (i, (name, node)) in self.children.iter().enumerate() {
            let last = i + 1 == self.children.len();
            output.push_str(&format!(
                "{}{}{}\n",
                prefix,
                if last { "└── " } else { "├── " },
                display_path(name)
            ));
            output.push_str(&node.render(&format!(
                "{}{}",
                prefix,
                if last { "    " } else { "│   " }
            )));
        }
        output
    }
}

struct SourceFile {
    path: PathBuf,
    content: String,
}

fn scan_dependencies(files: &[SourceFile]) -> String {
    let mut summary = String::new();
    // Derive metadata only from files actually included, after redaction. This also
    // respects filters, custom ignores, size limits, and interactive selection.
    for file in files {
        if file.path == Path::new("Cargo.toml") {
            if let Ok(cargo) = toml::from_str::<toml::Value>(&file.content) {
                let name = cargo
                    .get("package")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                summary.push_str(&format!("Project: {name} (Rust)\n"));
                if let Some(deps) = cargo.get("dependencies").and_then(|v| v.as_table()) {
                    summary.push_str("Dependencies (up to 15):\n");
                    for (key, value) in deps.iter().take(15) {
                        let version = value
                            .as_str()
                            .or_else(|| value.get("version").and_then(|v| v.as_str()))
                            .unwrap_or("*");
                        summary.push_str(&format!("  - {key}: {version}\n"));
                    }
                }
            }
        } else if file.path == Path::new("package.json") {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&file.content) {
                let name = pkg
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                summary.push_str(&format!("Project: {name} (Node.js)\n"));
                if let Some(deps) = pkg.get("dependencies").and_then(|v| v.as_object()) {
                    summary.push_str("Dependencies (up to 15):\n");
                    for (key, value) in deps.iter().take(15) {
                        summary.push_str(&format!(
                            "  - {}: {}\n",
                            key,
                            value.as_str().unwrap_or("*")
                        ));
                    }
                }
            }
        }
    }
    summary
}

pub fn scan(target: &Path, args: &crate::Args, config: &crate::Config) -> Result<()> {
    let root = if target.is_file() {
        target.parent().context("File has no parent")?
    } else {
        target
    };
    eprintln!(
        "{} Scanning target: {}",
        "[>>]".cyan().bold(),
        target.display()
    );
    let excluded_output = args.output.as_deref().map(output_identity).transpose()?;
    if excluded_output.as_deref() == Some(target) && target.is_file() {
        bail!("Output path must not overwrite the input file");
    }
    let pattern = args
        .filter
        .as_deref()
        .map(Pattern::new)
        .transpose()
        .context("Invalid glob pattern")?;
    let mut custom = GitignoreBuilder::new(root);
    for line in &config.ignore {
        custom
            .add_line(None, line)
            .with_context(|| format!("Invalid ignore pattern: {line}"))?;
    }
    let custom = custom.build().context("Invalid ignore configuration")?;
    let changed: Option<HashSet<PathBuf>> = if args.diff {
        Some(get_git_files(target)?.into_iter().collect())
    } else {
        None
    };
    let mut paths = get_walk_files(target, &custom)?;
    paths.retain(|path| {
        let relative = path.strip_prefix(root).expect("walk stays inside target");
        // Glob filters are target-relative. A filename-only glob also works in subdirectories.
        let matches = pattern.as_ref().is_none_or(|p| p.matches_path(relative));
        matches
            && !custom.matched_path_or_any_parents(path, false).is_ignore()
            && changed.as_ref().is_none_or(|files| files.contains(path))
            && excluded_output.as_ref().is_none_or(|output| {
                path != output && path.canonicalize().ok().as_ref() != Some(output)
            })
    });
    paths.sort();
    paths.dedup();
    if args.interactive && !paths.is_empty() {
        use dialoguer::{theme::ColorfulTheme, MultiSelect};
        let labels: Vec<_> = paths
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().display().to_string())
            .collect();
        let selection = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select files to include (Space to toggle, Enter to confirm)")
            .items(&labels)
            .defaults(&vec![true; labels.len()])
            .interact()
            .context("Failed to run interactive selection")?;
        if selection.is_empty() {
            eprintln!("No files selected. Exiting.");
            return Ok(());
        }
        paths = selection.into_iter().map(|i| paths[i].clone()).collect();
        paths.sort();
    }
    let bpe = cl100k_base()?;
    let scanner = SecretScanner::new()?;
    let processed: Vec<Result<Option<SourceFile>>> = paths
        .par_iter()
        .map(|path| {
            process_file(path, &scanner, args.max_size).map(|content| {
                content.map(|content| SourceFile {
                    path: path.strip_prefix(root).unwrap().to_path_buf(),
                    content,
                })
            })
        })
        .collect();
    let files: Vec<SourceFile> = processed
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    let output = render_output(&files, &bpe, args.format, args.numbers);
    let tokens = bpe.encode_ordinary(&output).len();
    let chars = output.chars().count();
    if let Some(path) = &args.output {
        write_output(Path::new(path), &output)?;
        eprintln!("{} Output written to: {}", "[OK]".green().bold(), path);
    }
    if args.copy {
        let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
        clipboard
            .set_text(&output)
            .context("Failed to copy output to clipboard")?;
        eprintln!(
            "{} Copied {} files, {} characters",
            "[OK]".green().bold(),
            files.len(),
            chars
        );
    }
    if args.output.is_none() && !args.copy {
        std::io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .context("Failed to write stdout")?;
    }
    eprintln!(
        "{} Payload Metrics: {} files | {} tokens | {} chars",
        "[i]".cyan().bold(),
        files.len(),
        tokens,
        chars
    );
    Ok(())
}

fn output_identity(path: &str) -> Result<PathBuf> {
    let path = Path::new(path);
    if path.exists() {
        return path
            .canonicalize()
            .with_context(|| format!("Cannot resolve output: {}", path.display()));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(parent
        .canonicalize()
        .context("Output directory does not exist")?
        .join(path.file_name().context("Output must name a file")?))
}

fn write_output(path: &Path, content: &str) -> Result<()> {
    // Complete the payload before replacing an existing output, avoiding partial exports.
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).context("Failed to create output file")?;
    temporary
        .write_all(content.as_bytes())
        .context("Failed to write output file")?;
    temporary.flush().context("Failed to flush output file")?;
    temporary
        .persist(path)
        .context("Failed to replace output file")?;
    Ok(())
}

fn render_output(
    files: &[SourceFile],
    bpe: &tiktoken_rs::CoreBPE,
    format: crate::OutputFormat,
    numbers: bool,
) -> String {
    let summary = scan_dependencies(files);
    let mut tree = TreeNode::default();
    for file in files {
        tree.insert(&file.path);
    }
    let tree = format!(".\n{}", tree.render(""));
    let mut output = String::new();
    match format {
        crate::OutputFormat::Xml => {
            output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<codebase>\n");
            if !summary.is_empty() {
                output.push_str(&format!(
                    "  <project-context>{}</project-context>\n",
                    escape_xml(&summary)
                ));
            }
            output.push_str(&format!(
                "  <structure>{}</structure>\n  <files>\n",
                escape_xml(&tree)
            ));
        }
        crate::OutputFormat::Markdown => {
            if !summary.is_empty() {
                output.push_str(&format!("# Project context\n\n{}\n", fenced(&summary)));
            }
            output.push_str(&format!(
                "# Project structure\n\n{}\n# File contents\n\n",
                fenced(&tree)
            ));
        }
    }
    for file in files {
        let content = if numbers {
            number_lines(&file.content)
        } else {
            file.content.clone()
        };
        let content = if matches!(format, crate::OutputFormat::Xml) {
            content
                .chars()
                .map(|c| if is_xml_char(c) { c } else { '\u{fffd}' })
                .collect()
        } else {
            content
        };
        let count = bpe.encode_ordinary(&content).len();
        let path = file.path.to_string_lossy();
        match format {
            crate::OutputFormat::Xml => output.push_str(&format!(
                "    <file path=\"{}\" tokens=\"{}\">{}</file>\n",
                escape_xml(&path),
                count,
                escape_xml(&content)
            )),
            crate::OutputFormat::Markdown => output.push_str(&format!(
                "## File: {} ({} tokens)\n\n{}\n",
                escape_markdown(&display_path(&path)),
                count,
                fenced(&content)
            )),
        }
    }
    if matches!(format, crate::OutputFormat::Xml) {
        output.push_str("  </files>\n</codebase>\n");
    }
    output
}

fn number_lines(content: &str) -> String {
    content
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, line)| format!("{:>4} | {}", i + 1, line))
        .collect()
}

fn fenced(content: &str) -> String {
    let longest = content.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(3.max(longest + 1));
    format!(
        "{}\n{}{}{}\n",
        fence,
        content,
        if content.ends_with('\n') { "" } else { "\n" },
        fence
    )
}

fn display_path(path: &str) -> String {
    path.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

fn escape_markdown(value: &str) -> String {
    value
        .chars()
        .flat_map(|c| {
            if r"\`*_{}[]<>()#+-.!|".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn escape_xml(value: &str) -> String {
    let mut escaped = String::new();
    for c in value.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\n' => escaped.push_str("&#10;"),
            '\r' => escaped.push_str("&#13;"),
            '\t' => escaped.push_str("&#9;"),
            '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}' => {
                escaped.push(c)
            }
            _ => escaped.push('\u{fffd}'),
        }
    }
    escaped
}

fn git(directory: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .context("Failed to execute git")
}

fn get_git_files(target: &Path) -> Result<Vec<PathBuf>> {
    let directory = if target.is_file() {
        target.parent().context("File has no parent")?
    } else {
        target
    };
    let check = git(directory, &["rev-parse", "--show-toplevel"])?;
    if !check.status.success() {
        bail!(
            "Target is not a Git working tree: {}",
            String::from_utf8_lossy(&check.stderr).trim()
        );
    }
    let has_head = git(directory, &["rev-parse", "--verify", "HEAD"])?
        .status
        .success();
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--name-only",
        "-z",
        "--diff-filter=ACMRTU",
        "--relative",
    ];
    if has_head {
        args.push("HEAD");
    } else {
        args.push("--cached");
    }
    args.extend(["--", "."]);
    let output = git(directory, &args)?;
    if !output.status.success() {
        bail!(
            "Git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut files = Vec::new();
    for name in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        #[cfg(unix)]
        let name = {
            use std::os::unix::ffi::OsStrExt;
            std::ffi::OsStr::from_bytes(name)
        };
        #[cfg(not(unix))]
        let name = std::ffi::OsStr::new(
            std::str::from_utf8(name).context("Git returned a non-UTF-8 filename")?,
        );
        let path = directory.join(name);
        if !target.is_file() || path == target {
            files.push(path);
        }
    }
    Ok(files)
}

fn get_walk_files(target: &Path, custom: &Gitignore) -> Result<Vec<PathBuf>> {
    let custom = custom.clone();
    let walker = WalkBuilder::new(target)
        .standard_filters(true)
        .follow_links(false)
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
            if custom
                .matched_path_or_any_parents(entry.path(), is_dir)
                .is_ignore()
            {
                return false;
            }
            !entry.file_type().is_some_and(|kind| kind.is_dir())
                || !matches!(
                    entry.file_name().to_str(),
                    Some(
                        "node_modules" | ".git" | "target" | "dist" | "build" | "vendor" | ".next"
                    )
                )
        })
        .build();
    let mut files = Vec::new();
    for entry in walker {
        let entry = entry.context("Failed to traverse target")?;
        if let Some(error) = entry.error() {
            bail!("Invalid ignore rules: {error}");
        }
        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

fn process_file(path: &Path, scanner: &SecretScanner, max_size: u64) -> Result<Option<String>> {
    // Check without following symlinks; linked sources can lead outside the selected tree.
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("Cannot inspect {}", path.display()))?;
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > max_size {
        eprintln!(
            "Skipping large file: {} (> {} bytes)",
            path.display(),
            max_size
        );
        return Ok(None);
    }
    let file = File::open(path).with_context(|| format!("Cannot read {}", path.display()))?;
    let mut bytes = Vec::new();
    // A file may grow after metadata was read. Bound the actual read too.
    file.take(max_size.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("Cannot read {}", path.display()))?;
    if bytes.len() as u64 > max_size {
        eprintln!(
            "Skipping large file: {} (> {} bytes)",
            path.display(),
            max_size
        );
        return Ok(None);
    }
    if bytes.contains(&0) {
        eprintln!("Skipping binary file: {}", path.display());
        return Ok(None);
    }
    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("Skipping non-UTF-8 file: {}", path.display());
            return Ok(None);
        }
    };
    Ok(Some(scanner.scan(&content, path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn redact(content: &str) -> String {
        static SCANNER: OnceLock<SecretScanner> = OnceLock::new();
        SCANNER
            .get_or_init(|| SecretScanner::new().unwrap())
            .scan(content, Path::new("fixture"))
    }

    #[test]
    fn redacts_complete_provider_keys_including_old_suffixes() {
        let old = format!("sk-{}T3BlbkFJ{}", "a".repeat(24), "b".repeat(20));
        for key in [
            old,
            format!("sk-proj-{}", "ab_C-012".repeat(12)),
            format!("sk-svcacct-{}", "d".repeat(40)),
        ] {
            assert_eq!(
                redact(&format!("prefix {key} suffix")),
                "prefix [REDACTED_OPENAI_KEY] suffix"
            );
        }
        for prefix in ["AKIA", "ASIA"] {
            assert_eq!(
                redact(&format!("{prefix}{}", "Z".repeat(16))),
                "[REDACTED_AWS_KEY]"
            );
        }
    }

    #[test]
    fn redacts_json_env_punctuation_unicode_and_short_values() {
        for (input, expected) in [
            (
                r#"{"api_key": "abcd!efgh+=/0123"}"#,
                r#"{"api_key": "[REDACTED_SECRET]"}"#,
            ),
            (
                "AUTH_TOKEN=abc.DEF-12_345+/== # note",
                "AUTH_TOKEN=[REDACTED_SECRET] # note",
            ),
            ("password = '短い秘密'", "password = '[REDACTED_SECRET]'"),
            ("password = 'abc'", "password = '[REDACTED_SECRET]'"),
            ("secret = 'secret'", "secret = '[REDACTED_SECRET]'"),
            (
                r#"{"password":"abc\"secret-tail", "ok":true}"#,
                r#"{"password":"[REDACTED_SECRET]", "ok":true}"#,
            ),
            (
                r"secret = 'abc\'secret-tail'",
                "secret = '[REDACTED_SECRET]'",
            ),
        ] {
            assert_eq!(redact(input), expected, "{input}");
        }
    }

    #[test]
    fn leaves_empty_secrets_and_unrelated_identifiers_alone() {
        for input in [
            "password = ''",
            "secret = \"\"",
            "mysecret = 'hello world'",
            "secret_count = 5",
            "ordinary text",
        ] {
            assert_eq!(redact(input), input);
        }
    }

    #[test]
    fn redacts_multiline_private_keys_without_eating_neighbors() {
        let input = "before\n-----BEGIN RSA PRIVATE KEY-----\nSYNTHETIC\n-----END RSA PRIVATE KEY-----\nafter";
        assert_eq!(redact(input), "before\n[REDACTED_PRIVATE_KEY]\nafter");
    }

    #[test]
    fn numbering_preserves_final_newline_and_crlf() {
        assert_eq!(number_lines(""), "");
        assert_eq!(number_lines("a\r\nb"), "   1 | a\r\n   2 | b");
        assert_eq!(number_lines("a\n\n"), "   1 | a\n   2 | \n");
    }

    #[test]
    fn xml_escapes_attributes_controls_and_carriage_returns() {
        assert_eq!(
            escape_xml("<&>\"'\r\n\t\u{1}\u{ffff}"),
            "&lt;&amp;&gt;&quot;&apos;&#13;&#10;&#9;\u{fffd}\u{fffd}"
        );
    }

    #[test]
    fn empty_selection_produces_complete_documents() {
        let bpe = cl100k_base().unwrap();
        let xml = render_output(&[], &bpe, crate::OutputFormat::Xml, false);
        assert!(xml.contains("  <files>\n  </files>\n</codebase>\n"));
        let markdown = render_output(&[], &bpe, crate::OutputFormat::Markdown, false);
        assert!(markdown.contains("# File contents"));
    }

    #[test]
    fn special_token_literals_are_counted_as_source_text() {
        let bpe = cl100k_base().unwrap();
        let content = "<|endoftext|>";
        let xml = render_output(
            &[SourceFile {
                path: "a.txt".into(),
                content: content.into(),
            }],
            &bpe,
            crate::OutputFormat::Xml,
            false,
        );
        let count = bpe.encode_ordinary(content).len();
        assert!(count > 1);
        assert!(xml.contains(&format!("tokens=\"{count}\"")));
    }
}
