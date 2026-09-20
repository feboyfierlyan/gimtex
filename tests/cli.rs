use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::{tempdir, TempDir};

fn fixture() -> TempDir {
    tempdir().unwrap()
}
fn write(root: &Path, name: &str, content: impl AsRef<[u8]>) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gimtex"))
        .current_dir(root)
        .args(args)
        .env("NO_COLOR", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap()
}
fn success(root: &Path, args: &[&str]) -> (String, String) {
    let output = run(root, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}
fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn repo() -> TempDir {
    let root = fixture();
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["config", "user.name", "Gimtex Test"]);
    git(
        root.path(),
        &["config", "user.email", "gimtex@example.invalid"],
    );
    root
}
fn commit(root: &Path) {
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "fixture"]);
}

#[test]
fn no_arguments_prints_help_but_filter_only_runs() {
    let root = fixture();
    write(root.path(), "src/a.rs", "FILTER_ONLY_CONTENT");
    write(root.path(), "other.txt", "EXCLUDED_CONTENT");
    let (help, _) = success(root.path(), &[]);
    assert!(help.contains("Usage:"));
    let (output, _) = success(root.path(), &["-i", "*.rs"]);
    assert!(output.contains("FILTER_ONLY_CONTENT"));
    assert!(!output.contains("EXCLUDED_CONTENT"));
}

#[test]
fn invalid_format_path_glob_and_output_fail() {
    let root = fixture();
    for args in [
        vec![".", "-f", "typo"],
        vec!["missing"],
        vec![".", "-i", "["],
        vec![".", "-o", "missing/out.md"],
    ] {
        let result = run(root.path(), &args);
        assert!(!result.status.success(), "{args:?} unexpectedly succeeded");
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn target_relative_filters_work_for_absolute_targets() {
    let root = fixture();
    let caller = fixture();
    write(root.path(), "src/a.rs", "INCLUDED_RUST");
    write(root.path(), "other/a.rs", "EXCLUDED_RUST");
    let (out, _) = success(
        caller.path(),
        &[root.path().to_str().unwrap(), "-i", "src/*.rs"],
    );
    assert!(out.contains("INCLUDED_RUST"));
    assert!(!out.contains("EXCLUDED_RUST"));
    assert!(!out.contains(root.path().to_str().unwrap()));
}

#[test]
fn target_config_and_gitignore_are_applied() {
    let root = repo();
    let caller = fixture();
    write(
        root.path(),
        "gimtex.toml",
        "ignore = ['private/', '*.log', '!keep.log']\n",
    );
    write(root.path(), ".gitignore", "ignored.txt\n");
    write(root.path(), "private/nested.txt", "CUSTOM_IGNORED_SECRET");
    write(root.path(), "debug.log", "LOG_IGNORED_SECRET");
    write(root.path(), "keep.log", "KEEP_LOG_CONTENT");
    write(root.path(), "ignored.txt", "GIT_IGNORED_SECRET");
    write(root.path(), "public.txt", "PUBLIC_CONTENT");
    write(caller.path(), "gimtex.toml", "invalid toml");
    let (out, _) = success(caller.path(), &[root.path().to_str().unwrap()]);
    for hidden in [
        "CUSTOM_IGNORED_SECRET",
        "LOG_IGNORED_SECRET",
        "GIT_IGNORED_SECRET",
    ] {
        assert!(!out.contains(hidden));
    }
    assert!(out.contains("KEEP_LOG_CONTENT"));
    assert!(out.contains("PUBLIC_CONTENT"));
}

#[test]
fn invalid_config_or_ignore_patterns_fail() {
    let root = fixture();
    for config in ["ignore = 2", "ignroe = []", "ignore = ['[z-a]']"] {
        write(root.path(), "gimtex.toml", config);
        let out = run(root.path(), &["."]);
        assert!(!out.status.success(), "{config}");
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn binary_non_utf8_and_oversized_files_are_absent_from_tree_and_count() {
    let root = fixture();
    write(root.path(), "ok.txt", "héllo 世界");
    write(root.path(), "large.txt", vec![b'x'; 2100]);
    let mut binary = vec![b'a'; 1500];
    binary.push(0);
    write(root.path(), "binary.bin", binary);
    write(root.path(), "invalid.txt", [0xff, 0xfe]);
    let (out, stderr) = success(root.path(), &[".", "--max-size", "2000"]);
    assert!(out.contains("héllo 世界"));
    for skipped in ["large.txt", "binary.bin", "invalid.txt"] {
        assert!(!out.contains(skipped));
    }
    assert!(stderr.contains("1 files"));
    assert!(stderr.contains(&format!("{} chars", out.chars().count())));
    let tokens = tiktoken_rs::cl100k_base()
        .unwrap()
        .encode_ordinary(&out)
        .len();
    assert!(stderr.contains(&format!("{tokens} tokens")));
}

#[test]
fn size_boundary_includes_empty_and_exact_limit_files() {
    let root = fixture();
    write(root.path(), "empty", "");
    write(root.path(), "exact", "12345");
    write(root.path(), "too_large", "123456");
    let (out, err) = success(root.path(), &[".", "--max-size", "5"]);
    assert!(out.contains("12345"));
    assert!(!out.contains("123456"));
    assert!(err.contains("2 files"));
    let (_, err) = success(root.path(), &[".", "--max-size", "0"]);
    assert!(err.contains("1 files"));
}

#[test]
fn repeated_exports_do_not_include_their_previous_output() {
    let root = fixture();
    write(root.path(), "a.txt", "SOURCE_CONTENT");
    write(root.path(), "context.md", "PREVIOUS_EXPORT");
    let (stdout, _) = success(root.path(), &[".", "-o", "context.md"]);
    assert!(stdout.is_empty());
    let first = fs::read_to_string(root.path().join("context.md")).unwrap();
    assert!(!first.contains("PREVIOUS_EXPORT"));
    assert!(!first.contains("context.md"));
    success(root.path(), &[".", "-o", "./context.md"]);
    assert_eq!(
        first,
        fs::read_to_string(root.path().join("context.md")).unwrap()
    );
}

#[test]
fn single_file_scan_has_name_and_cannot_overwrite_source() {
    let root = fixture();
    write(root.path(), "single.txt", "SOURCE_CONTENT");
    let (out, err) = success(root.path(), &["single.txt"]);
    assert!(out.contains("single"));
    assert!(out.contains("SOURCE_CONTENT"));
    assert!(err.contains("1 files"));
    assert!(!run(root.path(), &["single.txt", "-o", "single.txt"])
        .status
        .success());
    assert_eq!(
        fs::read_to_string(root.path().join("single.txt")).unwrap(),
        "SOURCE_CONTENT"
    );
}

#[test]
fn project_summary_respects_selection_and_redaction() {
    let root = fixture();
    write(
        root.path(),
        "package.json",
        r#"{"name":"sk-proj-abcdefghijklmnopqrstuvwxyz0123456789","dependencies":{"example":"1"}}"#,
    );
    write(root.path(), "a.rs", "SOURCE_CONTENT");
    let (filtered, _) = success(root.path(), &[".", "-i", "*.rs"]);
    assert!(!filtered.contains("Project context"));
    assert!(!filtered.contains("abcdefghijklmnopqrstuvwxyz"));
    let (all, _) = success(root.path(), &["."]);
    assert!(all.contains("Project context"));
    assert!(!all.contains("abcdefghijklmnopqrstuvwxyz"));
    write(root.path(), "gimtex.toml", "ignore = ['package.json']");
    let (ignored, _) = success(root.path(), &["."]);
    assert!(!ignored.contains("Project context"));
}

#[test]
fn diffs_use_target_repo_and_scope_to_subdirectory() {
    let root = repo();
    let caller = repo();
    write(root.path(), "src/changed.rs", "BEFORE");
    write(root.path(), "outside.rs", "BEFORE");
    write(root.path(), "src/deleted.rs", "DELETED_CONTENT");
    commit(root.path());
    write(root.path(), "src/changed.rs", "TARGET_CHANGED");
    write(root.path(), "outside.rs", "OUTSIDE_CHANGED");
    write(root.path(), "src/staged.rs", "TARGET_STAGED");
    write(root.path(), "src/untracked.rs", "UNTRACKED_CONTENT");
    git(root.path(), &["add", "src/staged.rs"]);
    fs::remove_file(root.path().join("src/deleted.rs")).unwrap();
    let target = root.path().join("src");
    let (out, err) = success(caller.path(), &[target.to_str().unwrap(), "--diff"]);
    assert!(out.contains("TARGET_CHANGED"));
    assert!(out.contains("TARGET_STAGED"));
    for excluded in ["OUTSIDE_CHANGED", "UNTRACKED_CONTENT", "DELETED_CONTENT"] {
        assert!(!out.contains(excluded));
    }
    assert!(err.contains("2 files"));
    let (single, err) = success(
        caller.path(),
        &[target.join("changed.rs").to_str().unwrap(), "--diff"],
    );
    assert!(single.contains("TARGET_CHANGED"));
    assert!(!single.contains("TARGET_STAGED"));
    assert!(err.contains("1 files"));
}

#[test]
fn diffs_support_first_commit_and_reject_non_repo() {
    let root = repo();
    write(root.path(), "new.txt", "NEWLY_STAGED");
    write(root.path(), "untracked.txt", "UNTRACKED_CONTENT");
    git(root.path(), &["add", "new.txt"]);
    let (out, err) = success(root.path(), &["--diff"]);
    assert!(out.contains("NEWLY_STAGED"));
    assert!(!out.contains("UNTRACKED_CONTENT"));
    assert!(err.contains("1 files"));
    let other = fixture();
    assert!(!run(other.path(), &[".", "--diff"]).status.success());
}

#[test]
fn diffs_apply_custom_ignores_and_filename_globs() {
    let root = repo();
    write(root.path(), "gimtex.toml", "ignore = ['private.rs']");
    write(root.path(), "private.rs", "BEFORE");
    write(root.path(), "a.rs", "BEFORE");
    write(root.path(), "a.txt", "BEFORE");
    commit(root.path());
    write(root.path(), "private.rs", "PRIVATE_CHANGED");
    write(root.path(), "a.rs", "RUST_CHANGED");
    write(root.path(), "a.txt", "TEXT_CHANGED");
    let (out, _) = success(root.path(), &[".", "--diff", "-i", "*.rs"]);
    assert!(out.contains("RUST_CHANGED"));
    assert!(!out.contains("PRIVATE_CHANGED"));
    assert!(!out.contains("TEXT_CHANGED"));
}

#[test]
fn diff_handles_renames_spaces_unicode_and_newlines() {
    let root = repo();
    write(root.path(), "old.txt", "BEFORE");
    commit(root.path());
    let name = if cfg!(windows) {
        "sp ace-é.txt"
    } else {
        "sp ace-é\nline.txt"
    };
    git(root.path(), &["mv", "old.txt", name]);
    write(root.path(), name, "RENAMED_CONTENT");
    let (out, err) = success(root.path(), &[".", "--diff"]);
    assert!(out.contains("RENAMED_CONTENT"));
    assert!(err.contains("1 files"));
}

#[test]
fn markdown_fences_protect_embedded_markdown_and_line_numbers() {
    let root = fixture();
    write(
        root.path(),
        "sample.md",
        "# heading\n```rust\nvalue\n```\nlast",
    );
    let (out, _) = success(root.path(), &[".", "-n"]);
    assert!(out.contains("````\n   1 | # heading\n"));
    assert!(out.contains("   5 | last\n````"));
}

#[test]
fn xml_is_a_single_escaped_document_and_preserves_text_boundaries() {
    let root = fixture();
    write(root.path(), "a&b.txt", "<&\"'>\r\nno final newline\u{1}");
    let (out, _) = success(root.path(), &[".", "-f", "xml"]);
    assert!(out.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<codebase>\n"));
    assert!(out.ends_with("</codebase>\n"));
    assert!(out.contains("path=\"a&amp;b.txt\""));
    assert!(out.contains("&lt;&amp;&quot;&apos;&gt;&#13;&#10;no final newline\u{fffd}</file>"));
    assert_eq!(out.matches("<file ").count(), 1);
}

#[test]
fn forced_terminal_colors_do_not_pollute_exported_payload() {
    let root = fixture();
    write(root.path(), "a.txt", "password = 'abc123'\n");
    let out = Command::new(env!("CARGO_BIN_EXE_gimtex"))
        .current_dir(root.path())
        .args([".", "-n", "-o", "context.md"])
        .env_remove("NO_COLOR")
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    assert!(out.status.success());
    let data = fs::read_to_string(root.path().join("context.md")).unwrap();
    assert!(!data.contains('\u{1b}'));
    assert!(!data.contains("abc123"));
    assert!(data.contains("[REDACTED_SECRET]"));
}

#[test]
fn directory_pruning_does_not_skip_regular_files_named_build_or_target() {
    let root = fixture();
    write(root.path(), "build", "REGULAR_FILE");
    write(root.path(), "node_modules/a.js", "VENDOR_CONTENT");
    let (out, _) = success(root.path(), &["."]);
    assert!(out.contains("REGULAR_FILE"));
    assert!(!out.contains("VENDOR_CONTENT"));
}

#[cfg(unix)]
#[test]
fn discovered_symlinks_do_not_read_outside_target() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    let outside = fixture();
    write(outside.path(), "private.txt", "OUTSIDE_SECRET");
    write(root.path(), "public.txt", "PUBLIC_CONTENT");
    symlink(
        outside.path().join("private.txt"),
        root.path().join("link.txt"),
    )
    .unwrap();
    symlink(outside.path(), root.path().join("linked_dir")).unwrap();
    let (out, err) = success(root.path(), &["."]);
    assert!(!out.contains("OUTSIDE_SECRET"));
    assert!(out.contains("PUBLIC_CONTENT"));
    assert!(err.contains("1 files"));
}

#[cfg(unix)]
#[test]
fn failed_remote_clone_returns_failure_without_network() {
    use std::os::unix::fs::PermissionsExt;
    let root = fixture();
    let bin = fixture();
    write(
        bin.path(),
        "git",
        "#!/bin/sh\nprintf 'simulated clone failure' >&2\nexit 1\n",
    );
    fs::set_permissions(bin.path().join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_gimtex"))
        .current_dir(root.path())
        .args(["https://example.invalid/repository"])
        .env("PATH", bin.path())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Git clone failed"));
    assert!(result.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn remote_clone_is_scanned_with_its_config_then_cleaned_up() {
    use std::os::unix::fs::PermissionsExt;
    let root = fixture();
    let bin = fixture();
    write(
        bin.path(),
        "git",
        r#"#!/bin/sh
[ "$1" = clone ] && [ "$2" = --depth ] && [ "$3" = 1 ] && [ "$4" = -- ] || exit 2
printf '%s' "$6" > "$CLONE_RECORD"
printf 'REMOTE_SOURCE_CONTENT' > "$6/source.rs"
printf 'REMOTE_PRIVATE_CONTENT' > "$6/private.txt"
printf "ignore = ['private.txt']" > "$6/gimtex.toml"
"#,
    );
    fs::set_permissions(bin.path().join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    let record = root.path().join("clone-path");
    let result = Command::new(env!("CARGO_BIN_EXE_gimtex"))
        .current_dir(root.path())
        .args(["https://example.invalid/repository"])
        .env("PATH", bin.path())
        .env("CLONE_RECORD", &record)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let out = String::from_utf8(result.stdout).unwrap();
    assert!(out.contains("REMOTE_SOURCE_CONTENT"));
    assert!(!out.contains("REMOTE_PRIVATE_CONTENT"));
    let clone = fs::read_to_string(record).unwrap();
    assert!(!out.contains(&clone));
    assert!(!Path::new(&clone).exists());
}

#[cfg(target_os = "linux")]
#[test]
fn diff_reads_non_utf8_filenames_without_crashing() {
    use std::os::unix::ffi::OsStrExt;
    let root = repo();
    let name = std::ffi::OsStr::from_bytes(b"file-\xff.txt");
    fs::write(root.path().join(name), "BEFORE").unwrap();
    commit(root.path());
    fs::write(root.path().join(name), "NON_UTF8_NAME_CONTENT").unwrap();
    let (out, err) = success(root.path(), &[".", "--diff"]);
    assert!(out.contains("NON_UTF8_NAME_CONTENT"));
    assert!(err.contains("1 files"));
}

#[cfg(unix)]
#[test]
fn output_symlink_alias_is_excluded_and_target_is_not_overwritten() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    let output_dir = fixture();
    write(root.path(), "old.md", "OLD_EXPORT");
    write(root.path(), "a.txt", "SOURCE_CONTENT");
    let alias = output_dir.path().join("context.md");
    symlink(root.path().join("old.md"), &alias).unwrap();
    success(root.path(), &[".", "-o", alias.to_str().unwrap()]);
    let out = fs::read_to_string(alias).unwrap();
    assert!(!out.contains("OLD_EXPORT"));
    assert!(out.contains("SOURCE_CONTENT"));
    assert_eq!(
        fs::read_to_string(root.path().join("old.md")).unwrap(),
        "OLD_EXPORT"
    );
}

#[test]
fn file_output_has_exactly_the_same_payload_as_stdout() {
    let root = fixture();
    let output_dir = fixture();
    write(root.path(), "a.txt", "a\r\nb\n<|endoftext|>");
    let path = output_dir.path().join("out.xml");
    let (stdout, _) = success(root.path(), &[".", "-f", "xml", "-n"]);
    success(
        root.path(),
        &[".", "-f", "xml", "-n", "-o", path.to_str().unwrap()],
    );
    assert_eq!(stdout, fs::read_to_string(path).unwrap());
}

#[test]
fn failed_scan_does_not_replace_previous_output() {
    let root = fixture();
    write(root.path(), "context.md", "PREVIOUS_EXPORT");
    write(root.path(), "gimtex.toml", "ignore = 2");
    assert!(!run(root.path(), &[".", "-o", "context.md"])
        .status
        .success());
    assert_eq!(
        fs::read_to_string(root.path().join("context.md")).unwrap(),
        "PREVIOUS_EXPORT"
    );
}

#[test]
fn explicitly_selected_build_directory_is_scanned() {
    let root = fixture();
    write(root.path(), "build/source.txt", "EXPLICIT_TARGET_CONTENT");
    let (out, _) = success(root.path(), &["build"]);
    assert!(out.contains("EXPLICIT_TARGET_CONTENT"));
}

#[test]
fn xml_file_tokens_count_the_normalized_content() {
    let root = fixture();
    write(root.path(), "a.txt", "a\u{ffff}b");
    let (out, _) = success(root.path(), &[".", "-f", "xml"]);
    let content = "a\u{fffd}b";
    let count = tiktoken_rs::cl100k_base()
        .unwrap()
        .encode_ordinary(content)
        .len();
    assert!(out.contains(&format!("tokens=\"{count}\">{content}</file>")));
}
