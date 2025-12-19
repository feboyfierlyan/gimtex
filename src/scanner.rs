use anyhow::{Result, Context};
use arboard::Clipboard;
use ignore::WalkBuilder;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tiktoken_rs::cl100k_base;
use glob::Pattern;
use colored::*;
use regex::Regex;
use std::collections::BTreeMap;

struct SecretScanner {
    generic_keys: Regex,
    openai_keys: Regex,
    aws_keys: Regex,
}

impl SecretScanner {
    fn new() -> Result<Self> {
        Ok(Self {
            generic_keys: Regex::new(r#"(?i)(api_?key|auth_?token|access_?key|secret|password)[\s]*[:=][\s]*['"](?P<secret>[a-zA-Z0-9_\-]{8,})['"]"#)?,
            openai_keys: Regex::new(r#"sk-[a-zA-Z0-9]{20,}T3BlbkFJ"#)?,
            aws_keys: Regex::new(r#"AKIA[0-9A-Z]{16}"#)?,
        })
    }

    fn scan(&self, content: &str, file_path: &Path) -> String {
        let mut sanitized = content.to_string();
        let mut found_secret = false;

        // Generic Keys
        if self.generic_keys.is_match(&sanitized) {
            sanitized = self.generic_keys.replace_all(&sanitized, |caps: &regex::Captures| {
                found_secret = true;
                let whole = caps.get(0).unwrap().as_str();
                let secret = caps.name("secret").unwrap().as_str();
                whole.replace(secret, &"[REDACTED_SECRET]".red().bold().to_string())
            }).to_string();
        }

        // OpenAI Keys
        if self.openai_keys.is_match(&sanitized) {
             found_secret = true;
             sanitized = self.openai_keys.replace_all(&sanitized, "[REDACTED_OPENAI_KEY]".red().bold().to_string().as_str()).to_string();
        }

        // AWS Keys
        if self.aws_keys.is_match(&sanitized) {
             found_secret = true;
             sanitized = self.aws_keys.replace_all(&sanitized, "[REDACTED_AWS_KEY]".red().bold().to_string().as_str()).to_string();
        }

        if found_secret {
            eprintln!("{} SECURITY ALERT: Potential secret found in file: {}", "[!]".red().bold(), file_path.display());
        }

        sanitized
    }
}

// Tree View Structures
struct TreeNode {
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn new() -> Self {
        Self { children: BTreeMap::new() }
    }

    fn insert(&mut self, path: &Path, _is_dir: bool) { 
        let components: Vec<_> = path.iter().collect();
        if components.is_empty() { return; }

        let mut current = self;
        for component in components {
            let name = component.to_string_lossy().to_string();
            current = current.children.entry(name).or_insert_with(TreeNode::new);
        }
    }

    fn render(&self, prefix: &str, _is_root: bool) -> String {
        let mut output = String::new();
        let count = self.children.len();
        for (i, (name, node)) in self.children.iter().enumerate() {
            let is_last = i == count - 1;
            let connector = if is_last { "└── " } else { "├── " };
            let child_prefix = if is_last { "    " } else { "│   " };
            
            // Visualization Logic: 
            let display_name = if node.children.is_empty() {
                name.white().to_string()
            } else {
                name.cyan().bold().to_string()
            };

            output.push_str(&format!("{}{}{}\n", prefix, connector, display_name));
            output.push_str(&node.render(&format!("{}{}", prefix, child_prefix), false));
        }
        output
    }
}

fn generate_tree_view(files: &[PathBuf], root: &str) -> String {
    let mut tree_root = TreeNode::new();
    let root_path = Path::new(root);

    for path in files {
        // Strip prefix to get relative path for the tree
        let relative_path = path.strip_prefix(root_path).unwrap_or(path);
        tree_root.insert(relative_path, false);
    }

    format!("{}\n{}", 
        root.cyan().bold(), // Root directory name
        tree_root.render("", true)
    )
}

use serde::Deserialize;

#[derive(Deserialize)]
struct CargoToml {
    package: Option<CargoPackage>,
    dependencies: Option<toml::Table>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
}

#[derive(Deserialize)]
struct PackageJson {
    name: Option<String>,
    dependencies: Option<serde_json::Map<String, serde_json::Value>>,
}

fn scan_dependencies(root: &str) -> Option<String> {
    let root_path = Path::new(root);
    let mut summary = String::new();

    // Strategy: robust parsing
    
    // Rust (Cargo.toml)
    if let Ok(content) = std::fs::read_to_string(root_path.join("Cargo.toml")) {
        if let Ok(cargo) = toml::from_str::<CargoToml>(&content) {
            let name = cargo.package.map(|p| p.name).unwrap_or("Unknown".to_string());
            summary.push_str(&format!("{} Project: {} (Rust)\n", "[+]".green(), name.bold()));
            
            if let Some(deps) = cargo.dependencies {
                summary.push_str(&format!("{} Dependencies:\n", "[+]".green()));
                // Limit to first 15 for brevity
                for (k, v) in deps.iter().take(15) {
                    // toml values can be complex (inline tables), we just want the version usually
                    let version = match v {
                        toml::Value::String(s) => s.clone(),
                        toml::Value::Table(t) => t.get("version").and_then(|v| v.as_str()).unwrap_or("*").to_string(),
                        _ => "*".to_string(),
                    };
                    summary.push_str(&format!("    - {}: {}\n", k, version.dimmed()));
                }
            }
        }
    }
    
    // Node.js (package.json)
    if let Ok(content) = std::fs::read_to_string(root_path.join("package.json")) {
        if let Ok(pkg) = serde_json::from_str::<PackageJson>(&content) {
             let name = pkg.name.unwrap_or("Unknown".to_string());
             summary.push_str(&format!("{} Project: {} (Node.js)\n", "[+]".green(), name.bold()));
             
             if let Some(deps) = pkg.dependencies {
                summary.push_str(&format!("{} Dependencies:\n", "[+]".green()));
                for (k, v) in deps.iter().take(15) {
                    let version = v.as_str().unwrap_or("*");
                    summary.push_str(&format!("    - {}: {}\n", k, version.dimmed()));
                }
             }
        }
    }

    if summary.is_empty() {
        None
    } else {
        Some(format!("PROJECT CONTEXT:\n================\n{}\n", summary))
    }
}

pub fn scan(path: &str, config: &crate::Args) -> Result<()> {
    eprintln!("{} Scanning target: {}", "[>>]".cyan().bold(), path.cyan());

    let bpe = cl100k_base()?;
    let scanner = SecretScanner::new()?;
    let mut output = String::new();
    
    // Strategy Selection
    let raw_files: Vec<PathBuf> = if config.diff {
        eprintln!("{} Git Intelligence Mode: Active", "[>>]".cyan().bold());
        get_git_files(path)?
    } else {
        get_walk_files(path)
    };

    // Filter Compilation
    let filter_pattern = match &config.filter {
        Some(p) => {
            eprintln!("{} Precision Filtering: {}", "[>>]".cyan().bold(), p.yellow());
            Some(Pattern::new(p).context("Invalid glob pattern")?)
        },
        None => None,
    };

    // Apply Filter & Collect final list for Tree + Processing
    let mut final_files = Vec::new();
    for p in raw_files {
        if let Some(ref pattern) = filter_pattern {
            if !pattern.matches_path(&p) {
                continue;
            }
        }
        final_files.push(p);
    }
    
    // Determinism: Sort files alphabetically
    final_files.sort();

    // Context Mapping sequence
    
    // 1. Recon Module (Project Context)
    if let Some(context_header) = scan_dependencies(path) {
        output.push_str(&context_header);
        output.push_str("\n");
    }

    // 2. Tree View
    let tree_view = generate_tree_view(&final_files, path);
    output.push_str("PROJECT STRUCTURE:\n==================\n");
    output.push_str(&tree_view);
    output.push_str("\n\nFILE CONTENTS:\n==================\n\n");

    let file_count = final_files.len();

    // PARALLEL PROCESSING
    // We Map files to their processed string output, then collect them IN ORDER.
    // rayon's `par_iter` combined with `map` and `collect` preserves order if used correctly,
    // but `collect::<Vec<_>>` definitely preserves it relative to the input iterator.
    use rayon::prelude::*;
    
    let processed_results: Vec<Option<(String, usize)>> = final_files
        .par_iter()
        .map(|path| process_file(path, &bpe, &scanner, config.numbers))
        .collect();

    // We use zip to iterate matching files and results.
    for (path, result) in final_files.iter().zip(processed_results.into_iter()) {
         if let Some((text, count)) = result {
            match config.format.as_str() {
                 "xml" => {
                    output.push_str(&format!("<file path=\"{}\" tokens=\"{}\">\n", path.display(), count));
                    output.push_str(&text);
                    output.push_str("\n</file>\n");
                }
                _ => { // markdown default
                     let header = format!("{} File: {} ({}) {}", 
                        "---".truecolor(100, 100, 100), 
                        path.display().to_string().yellow().bold(), 
                        format!("{} tokens", count).white().dimmed(),
                        "---".truecolor(100, 100, 100)
                    );
                    output.push_str(&header);
                    output.push_str("\n");
                    output.push_str(&text);
                    output.push_str("\n\n");
                }
            }
         }
    }

    // Tokenomics
    let final_token_count = bpe.encode_with_special_tokens(&output).len();

    // Output
    if config.copy {
        match Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(&output) {
                    eprintln!("{} Clipboard failure: {}", "[X]".red().bold(), e);
                } else {
                    eprintln!("{} Payload generated: {} files, {} chars copied.", 
                        "[OK]".green().bold(), 
                        file_count, 
                        output.len()
                    );
                }
            },
            Err(e) => eprintln!("{} Clipboard init failure: {}", "[X]".red().bold(), e),
        }
    } else {
        println!("{}", output);
    }
    
    // Dashboard
    print_dashboard(final_token_count, output.len());
    
    Ok(())
}

fn print_dashboard(tokens: usize, chars: usize) {
    let token_fmt = tokens.to_string();
    let char_fmt = chars.to_string();
    
    let token_color = if tokens < 30_000 {
        token_fmt.green().bold()
    } else if tokens < 100_000 {
        token_fmt.yellow().bold()
    } else {
        token_fmt.red().bold()
    };

    eprintln!("{} Payload Metrics: {} tokens | {} chars", 
        "[i]".cyan().bold(), 
        token_color, 
        char_fmt.white().bold()
    );
}

fn get_git_files(_path: &str) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .output()
        .context("Failed to execute git")?;
        
    if !output.status.success() {
        eprintln!("{} Git command failed", "[X]".red().bold());
        anyhow::bail!("Git command failed");
    }
    
    let content = String::from_utf8(output.stdout)?;
    let mut files = Vec::new();
    for line in content.lines() {
        let p = PathBuf::from(line);
        if p.exists() && p.is_file() {
            files.push(p);
        }
    }
    Ok(files)
}

fn get_walk_files(path: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(path)
        .standard_filters(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            // Aggressive Optimization: Prune massive folders at the discovery level
            if name == "node_modules" 
                || name == ".git" 
                || name == "target" 
                || name == "dist" 
                || name == "build"
                || name == "vendor"
                || name == ".next" {
                return false;
            }
            true
        })
        .build();

    for result in walker {
        match result {
            Ok(entry) => {
                if entry.path().is_file() {
                    files.push(entry.path().to_path_buf());
                }
            }
            Err(err) => {
                 eprintln!("{} Access Denied: {}", "[X]".red().bold(), err);
            }
        }
    }
    files
}

fn process_file(path: &Path, bpe: &tiktoken_rs::CoreBPE, scanner: &SecretScanner, show_numbers: bool) -> Option<(String, usize)> {
    // Binary check
     let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{} Skipping {}: {}", "[!]".yellow().bold(), path.display(), e);
            return None;
        }
    };

    let mut buffer = [0; 1024];
    let n = match file.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return None,
    };

    if buffer[..n].contains(&0) {
        eprintln!("{} Skipping binary file: {}", "[!]".yellow().bold(), path.display());
        return None;
    }

    let mut content = std::fs::read_to_string(path).ok()?;
    
    // Security Scan
    content = scanner.scan(&content, path);

    // Line Indexing (Optional)
    if show_numbers {
        let mut indexed_content = String::new();
        for (i, line) in content.lines().enumerate() {
            let line_num = format!("{:>4} |", i + 1);
            // We use standard colors explicitly or use colored crate but strip it for payload?
            // The user asked for "Visual Polish" in the output. 
            // If the user wants to copy to clipboard, colored codes might be annoying if pasting into an editor that doesn't support them.
            // However, the prompt says "Visual Polish" using `colored` crate.
            // Generally for LLM context, plain text is better.
            // But let's follow instruction: "make the line number... a distinct color... using the colored crate"
            // Wait, if it's for LLM context, ANSI codes are garbage.
            // BUT, the tool is a CLI for "Text EXtraction".
            // The `SecretScanner` effectively modifies the content.
            // Let's assume the user wants it visually in the terminal.
            // If `copy` is used, we might want to strip colors or keep them?
            // The `colored` crate normally respects NO_COLOR or non-tty, but if we format! it into a string, it bakes them in.
            // The redactor puts `[REDACTED]`.red().
            // If I bake ANSI codes into the payload, the LLM will see them.
            // Most LLMs can handle or ignore them, but it consumes tokens.
            // "Facilitate accurate debugging with LLMs" -> LLMs don't need ANSI colors for line numbers, they need the numbers.
            // The "Visual Polish" might be for the human review.
            // I will use `white().dimmed()` for the number part as requested.
            // If the user copies this, it will copy the ANSI codes.
            // This is a tradeoff. I will implement as requested.
            
            indexed_content.push_str(&format!("{} {}\n", line_num.white().dimmed(), line));
        }
        content = indexed_content;
    }

    let tokens = bpe.encode_with_special_tokens(&content);
    Some((content, tokens.len()))
}
