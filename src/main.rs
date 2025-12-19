mod scanner;

use anyhow::Result;
use clap::{Parser, CommandFactory, FromArgMatches};
use colored::*;

const BANNER: &str = r#"
  ____ ___ __  __ _____ _______  __
 / ___|_ _|  \/  |_   _| ____\ \/ /
| |  _ | || |\/| | | | |  _|  \  / 
| |_| || || |  | | | | | |___ /  \ 
 \____|___|_|  |_| |_| |_____/_/\_\
"#;

const TAGLINE: &str = ">> GIMTEX v1.0 :: Git-Integrated Module for Text EXtraction";

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
}

fn main() -> Result<()> {
    // Inject Cyber-Industrial Aesthetic
    let banner_colored = format!("{}\n{}", BANNER.cyan().bold(), TAGLINE.white().italic());
    let examples_colored = EXAMPLES.yellow(); // Making examples Yellow for contrast

    // Build command manually to inject colored help
    let command = Args::command()
        .before_help(banner_colored)
        .after_help(examples_colored.to_string());
        
    let matches = command.get_matches();
    let args = Args::from_arg_matches(&matches)?;

    // Logic hook
    // Safety: If no path is provided AND --diff is not set, we default to printing help
    // rather than scanning the current directory to prevent accidental huge scans.
    if args.path.is_none() && !args.diff {
        let mut cmd = Args::command()
            .before_help(format!("{}\n{}", BANNER.cyan().bold(), TAGLINE.white().italic()))
            .after_help(examples_colored.to_string());
        cmd.print_help()?;
        return Ok(());
    }

    let target_path = args.path.as_deref().unwrap_or(".");
    scanner::scan(target_path, &args)?;

    Ok(())
}
