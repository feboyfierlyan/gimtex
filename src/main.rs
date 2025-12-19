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

const TAGLINE: &str = ">> GIMTEX v2.2 :: Git-Integrated Module for Text EXtraction";

const EXAMPLES: &str = "
EXAMPLES:
  gimtex .                    # Dump current dir to stdout
  gimtex -c                   # Dump & copy to clipboard (Silent Mode)
  gimtex src/ -i \"*.rs\"       # Dump only Rust files in src/
  gimtex --diff --xml         # Dump git changes in XML format (Claude optimized)
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
    // Safety: If no path is provided AND --diff is not set, we default to printing help
    if args.path.is_none() && !args.diff {
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

    let target_path = args.path.as_deref().unwrap_or(".");
    scanner::scan(target_path, &args)?;

    Ok(())
}
