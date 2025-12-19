mod scanner;

use anyhow::{Result, Context};
use clap::{Parser, CommandFactory, FromArgMatches};
use colored::Colorize;
use std::fs;
use serde::Deserialize;
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the directory to scan
    #[arg()]
    path: Option<String>,

    /// Copy output to clipboard
    #[arg(short, long)]
    copy: bool,

    /// Output format (markdown, xml)
    #[arg(short, long, default_value = "markdown")]
    format: String,

    /// Filter files by glob pattern (e.g. "*.rs")
    #[arg(short = 'i', long)]
    filter: Option<String>,

    /// Only extract files changed/staged in git
    #[arg(short, long)]
    diff: bool,

    /// Add line numbers to output
    #[arg(short = 'n', long)]
    numbers: bool,

    /// Output to file instead of stdout
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Maximum file size in bytes to process (default: 100KB)
    #[arg(long, default_value_t = 100_000)]
    max_size: u64,

    /// Interactive mode: Select files manually
    #[arg(short = 'I', long)]
    interactive: bool,
}

#[derive(Debug, Deserialize)]
struct Config {
    ignore: Option<Vec<String>>,
    // We can add more config fields here later
}

fn load_config() -> Option<Config> {
    let config_path = Path::new("gimtex.toml");
    if config_path.exists() {
         let content = fs::read_to_string(config_path).ok()?;
         toml::from_str(&content).ok()
    } else {
        None
    }
}

fn main() -> Result<()> {
    // Inject Cyber-Industrial Aesthetic
    let banner_colored = format!("{}\n{}", BANNER.cyan().bold(), TAGLINE.white().italic());
    let examples_colored = EXAMPLES.yellow(); 

    // Build command manually to inject colored help
    let command = Args::command()
        .before_help(banner_colored)
        .after_help(examples_colored.to_string());
        
    let matches = command.get_matches();
    let mut args = Args::from_arg_matches(&matches)?;

    // Logic hook
    // Safety: If no path is provided AND --diff is not set AND --interactive is not set, we default to printing help
    if args.path.is_none() && !args.diff && !args.interactive {
        let mut cmd = Args::command()
            .before_help(format!("{}\n{}", BANNER.cyan().bold(), TAGLINE.white().italic()))
            .after_help(examples_colored.to_string());
        cmd.print_help()?;
        return Ok(());
    }

    // Config Merge Strategy:
    // If gimtex.toml exists, we might someday merge ignore patterns etc.
    // For now, let's just log if we found it to verify the architecture.
    if let Some(cfg) = load_config() {
        // In the future, we will pass this config to scanner.
        // For now, we will just print that we loaded it to confirm Phase 14 success.
        eprintln!("{} Config loaded: gimtex.toml", "[>>]".cyan().bold());
        if let Some(ignores) = cfg.ignore {
             eprintln!("{} Custom Ignores: {:?}", "[>>]".cyan().bold(), ignores);
             // TODO: Pass these to scanner in a future update or refactor Args to include them
        }
    }

    let mut target_path_buf = std::path::PathBuf::from(args.path.as_deref().unwrap_or("."));

    // REMOTE SCOUT PROTOCOL
    let target_str = args.path.as_deref().unwrap_or(".");
    let temp_dir; // Keep alive scope

    if target_str.starts_with("http") || target_str.starts_with("git@") {
        use std::process::Command;
        use indicatif::{ProgressBar, ProgressStyle};

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap());
        spinner.set_message(format!("Locating Remote Target: {}", target_str));
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        // Create Temp Dir
        temp_dir = tempfile::Builder::new()
            .prefix("gimtex_remote")
            .tempdir()
            .context("Failed to create temporary bunker")?;
        
        spinner.set_message("Cloning Data Stream...");

        // Git Clone
        let status = Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1") // Shallow clone for speed
            .arg(target_str)
            .arg(temp_dir.path())
            .output()
            .context("Failed to execute git clone")?;

        if !status.status.success() {
            spinner.finish_with_message(format!("{} Connection Failed", "[X]".red()));
            eprintln!("{}", String::from_utf8_lossy(&status.stderr));
            return Ok(());
        }

        spinner.finish_with_message(format!("{} Target Acquired", "[OK]".green()));
        target_path_buf = temp_dir.path().to_path_buf();
    }

    scanner::scan(target_path_buf.to_str().unwrap(), &args)?;

    Ok(())
}
