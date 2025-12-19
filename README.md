<div align="center">

<img src="assets/banner_shot.png" alt="GIMTEX Banner" width="600px">

# GIMTEX (Git-Integrated Module for Text EXtraction)

![Rust](https://img.shields.io/badge/language-Rust-orange?style=for-the-badge&logo=rust)
![License](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge)
![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?style=for-the-badge)

> **A blazingly fast command-line utility to pack, filter, and sanitize codebases for Large Language Model (LLM) context.**

</div>

---

## The Why?

**Stop manually copy-pasting files to ChatGPT.**  
Stop hitting token limits because you pasted `package-lock.json`.  
Stop leaking your AWS keys in a rush to debug.

**Gimtex** is the bridge between your local codebase and your AI assistant. It converts your project into a highly optimized, clean, and safe context payload in milliseconds.

### Scenarios

#### 1. The Standard Run
Dump the entire project (respecting `.gitignore`) to stdout.
```bash
gimtex .
```

#### 2. The "Quick Fix"
You broke the build. You just need the AI to see your recent changes.
```bash
gimtex --diff --copy
```

#### 3. The "Deep Dive"
Debugging a specific module with line numbers for precise AI references.
```bash
gimtex src/scanner.rs --numbers --copy
```

#### 4. The "Architect"
Generating a massive context payload for Claude (XML format) to refactor the entire `src` folder.
```bash
gimtex src/ --format xml > context.xml
```

## Roadmap

- [ ] **Config File**: `.gimtexignore` or `.gimtex.toml` for persistent settings.
- [ ] **Web Interface**: A localhost GUI for selecting files visually.
- [ ] **Remote Repos**: `gimtex https://github.com/user/repo` support.

---
*Built for the Age of AI.*
