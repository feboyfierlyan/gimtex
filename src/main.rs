mod scanner;

use anyhow::{Context, Result};
use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};
use colored::Colorize;
use serde::Deserialize;
use std::fs;
use std::path::Path;

const BANNER: &str = r#"
  ____ ___ __  __ _____ _______  __
 / ___|_ _|  \/  |_   _| ____\ \/ /
| |  _ | || |\/| | | | |  _|  \  / 
| |_| || || |  | | | | | |___ /  \ 
 \____|___|_|  |_| |_| |_____/_/\_\
"#;

const TAGLINE: &str = ">> GIMTEX v2.6 :: Git-Integrated Module for Text EXtraction";

const EXAMPLES: &str = "
EXAMPLES:
  gimtex .                        # Standard: Scan current directory
  gimtex -I                       # Interactive: Cherry-pick files
  gimtex -i \"*.rs\"                # Filter: Scan only Rust files
  gimtex src/ -I -o context.md    # Combo: Interactive + Save to file
  gimtex https://github.com/user/repo -I  # Remote: Clone & Interactive scan
";

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Markdown,
    Xml,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// File, directory, or repository URL to scan (defaults to current directory)
    #[arg()]
    path: Option<String>,

    /// Copy output to clipboard
    #[arg(short, long)]
    copy: bool,

    /// Output format (markdown, xml)
    #[arg(short, long, value_enum, default_value = "markdown")]
    format: OutputFormat,

    /// Filter files by glob pattern (e.g. "*.rs")
    #[arg(short = 'i', long)]
    filter: Option<String>,

    /// Extract tracked files changed/staged against HEAD (excludes untracked files)
    #[arg(short, long)]
    diff: bool,

    /// Add line numbers to output
    #[arg(short = 'n', long)]
    numbers: bool,

    /// Output to file instead of stdout (can be combined with --copy)
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Maximum file size in bytes to process (default: 100KB)
    #[arg(long, default_value_t = 100_000)]
    max_size: u64,

    /// Interactive mode: Select files manually
    #[arg(short = 'I', long)]
    interactive: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    ignore: Vec<String>,
}

fn load_config(root: &Path) -> Result<Config> {
    let config_path = root.join("gimtex.toml");
    match fs::read_to_string(&config_path) {
        Ok(content) => toml::from_str(&content)
            .with_context(|| format!("Invalid configuration: {}", config_path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to read configuration: {}", config_path.display())),
    }
}

fn main() -> Result<()> {
    let banner_colored = format!("{}\n{}", BANNER.cyan().bold(), TAGLINE.white().italic());
    let examples_colored = EXAMPLES.yellow();

    // Build command manually to inject colored help
    let command = Args::command()
        .before_help(banner_colored)
        .after_help(examples_colored.to_string());

    let matches = command.get_matches();
    let args = Args::from_arg_matches(&matches)?;

    // Bare invocation shows help; any explicit option scans the current directory.
    if std::env::args_os().len() == 1 {
        let mut cmd = Args::command()
            .before_help(format!(
                "{}\n{}",
                BANNER.cyan().bold(),
                TAGLINE.white().italic()
            ))
            .after_help(examples_colored.to_string());
        cmd.print_help()?;
        return Ok(());
    }

    let mut target_path_buf = std::path::PathBuf::from(args.path.as_deref().unwrap_or("."));

    // Keep the temporary clone alive until extraction and output complete.
    let target_str = args.path.as_deref().unwrap_or(".");
    let temp_dir; // Keep alive scope

    if !target_path_buf.exists() && is_remote(target_str) {
        use indicatif::{ProgressBar, ProgressStyle};
        use std::process::Command;

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        spinner.set_message(format!("Locating Remote Target: {}", target_str));
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        // Create Temp Dir
        temp_dir = tempfile::Builder::new()
            .prefix("gimtex_remote")
            .tempdir()
            .context("Failed to create temporary clone directory")?;

        spinner.set_message("Cloning Data Stream...");

        // Git Clone
        let status = Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1") // Shallow clone for speed
            .arg("--")
            .arg(target_str)
            .arg(temp_dir.path())
            .output()
            .context("Failed to execute git clone")?;

        if !status.status.success() {
            spinner.finish_with_message(format!("{} Connection Failed", "[X]".red()));
            anyhow::bail!(
                "Git clone failed: {}",
                String::from_utf8_lossy(&status.stderr).trim()
            );
        }

        spinner.finish_with_message(format!("{} Target Acquired", "[OK]".green()));
        target_path_buf = temp_dir.path().to_path_buf();
    }

    let target = target_path_buf
        .canonicalize()
        .with_context(|| format!("Cannot access target: {}", target_path_buf.display()))?;
    anyhow::ensure!(
        target.is_file() || target.is_dir(),
        "Target must be a regular file or directory"
    );
    let root = if target.is_file() {
        target.parent().context("File has no parent")?
    } else {
        &target
    };
    let config = load_config(root)?;
    scanner::scan(&target, &args, &config)?;

    Ok(())
}

fn is_remote(path: &str) -> bool {
    ["https://", "http://", "ssh://", "git://", "git@"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_git_urls_without_mistaking_http_named_directories() {
        for url in [
            "https://example.invalid/repo",
            "http://example.invalid/repo",
            "git@example.invalid:repo",
            "ssh://example.invalid/repo",
            "git://example.invalid/repo",
        ] {
            assert!(is_remote(url));
        }
        for path in ["http-server", "https", "src/", "."] {
            assert!(!is_remote(path));
        }
    }

    #[test]
    fn output_and_clipboard_can_be_requested_together() {
        let args = Args::try_parse_from(["gimtex", "-c", "-o", "context.md"]).unwrap();
        assert!(args.copy);
        assert_eq!(args.output.as_deref(), Some("context.md"));
    }
}
