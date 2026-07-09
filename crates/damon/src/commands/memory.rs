//! `damon memory` — print or edit an agent's canonical memory files.
//! Print-free core (files/resolve_file) + printing CLI wrapper (run).

use std::path::{Path, PathBuf};

use damon_core::config::Config;
use damon_core::store::Store;

/// Memory surfaces in display order: the three seeded files, then every file
/// under skills/ sorted by relative path. Returns (path relative to the
/// memory dir, content).
pub fn files(memory_dir: &Path) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    for f in ["AGENT.md", "USER.md", "MEMORY.md"] {
        let path = memory_dir.join(f);
        if path.is_file() {
            out.push((PathBuf::from(f), std::fs::read_to_string(&path)?));
        }
    }
    let mut skills = Vec::new();
    collect_files(&memory_dir.join("skills"), memory_dir, &mut skills)?;
    skills.sort_by(|a, b| a.0.cmp(&b.0));
    out.append(&mut skills);
    Ok(out)
}

fn collect_files(dir: &Path, base: &Path, out: &mut Vec<(PathBuf, String)>) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(anyhow::anyhow!("{}: {e}", dir.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|e| anyhow::anyhow!("{}: {e}", dir.display()))?;
        // file_type() does NOT follow symlinks: a symlinked dir is neither
        // is_dir() nor is_file() here, so it is skipped — no cycle risk.
        let ft = entry
            .file_type()
            .map_err(|e| anyhow::anyhow!("{}: {e}", entry.path().display()))?;
        let path = entry.path();
        if ft.is_dir() {
            collect_files(&path, base, out)?;
        } else if ft.is_file() {
            let rel = path.strip_prefix(base).unwrap_or(&path).to_path_buf();
            out.push((rel, std::fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

/// Resolve FILE inside the memory dir. Canonicalization enforces existence
/// and rejects `..`, absolute paths, and symlinks escaping the memory dir.
pub fn resolve_file(memory_dir: &Path, file: &str) -> anyhow::Result<PathBuf> {
    let dir = memory_dir
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("memory dir {}: {e}", memory_dir.display()))?;
    let candidate = memory_dir.join(file);
    let path = candidate
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("no memory file {}: {e}", candidate.display()))?;
    if !path.starts_with(&dir) {
        anyhow::bail!("{file:?} escapes the memory directory");
    }
    Ok(path)
}

pub fn run(reference: &str, file: Option<&str>, edit: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let store = Store::new(config.root()?);
    let entry = store.resolve(reference)?;
    let dir = store.memory_dir(&entry.team, &entry.slug);
    if !dir.is_dir() {
        anyhow::bail!(
            "no memory directory for {reference} at {} — the agent is broken; recreate it",
            dir.display()
        );
    }
    if edit {
        let path = resolve_file(&dir, file.unwrap_or("MEMORY.md"))?;
        return edit_file(&path);
    }
    match file {
        Some(f) => {
            let path = resolve_file(&dir, f)?;
            print!("{}", std::fs::read_to_string(&path)?);
        }
        None => {
            for (rel, content) in files(&dir)? {
                println!("── {} ──", rel.display());
                print!("{content}");
                if !content.ends_with('\n') {
                    println!();
                }
            }
        }
    }
    Ok(())
}

/// $VISUAL, then $EDITOR, then vi. Empty values count as unset.
fn editor_from(visual: Option<&str>, editor: Option<&str>) -> String {
    [visual, editor]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "vi".to_string())
}

/// Spawn the editor inheriting the TTY; damon exits with the editor's code.
/// Editor values may carry flags ("code -w"), so split on whitespace.
fn edit_file(path: &Path) -> anyhow::Result<()> {
    let editor = editor_from(
        std::env::var("VISUAL").ok().as_deref(),
        std::env::var("EDITOR").ok().as_deref(),
    );
    let mut parts = editor.split_whitespace();
    let program = parts.next().expect("editor_from never returns empty");
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(path)
        .status()
        .map_err(|e| anyhow::anyhow!("cannot launch editor {editor:?}: {e}"))?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (name, content) in [
            ("AGENT.md", "# Scout\n"),
            ("USER.md", "user\n"),
            ("MEMORY.md", "memory\n"),
        ] {
            std::fs::write(tmp.path().join(name), content).unwrap();
        }
        let skill = tmp.path().join("skills/research");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "skill\n").unwrap();
        tmp
    }

    #[test]
    fn files_lists_surfaces_in_display_order() {
        let tmp = memory_fixture();
        let files = files(tmp.path()).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                "AGENT.md",
                "USER.md",
                "MEMORY.md",
                "skills/research/SKILL.md"
            ]
        );
        assert_eq!(files[0].1, "# Scout\n");
    }

    #[test]
    fn resolve_file_accepts_nested_relative_paths() {
        let tmp = memory_fixture();
        let p = resolve_file(tmp.path(), "skills/research/SKILL.md").unwrap();
        assert!(p.ends_with("skills/research/SKILL.md"));
    }

    #[test]
    fn resolve_file_rejects_traversal_absolute_and_symlink_escape() {
        let holder = tempfile::tempdir().unwrap();
        let mem = holder.path().join("memory");
        std::fs::create_dir_all(&mem).unwrap();
        std::fs::write(mem.join("MEMORY.md"), "m").unwrap();
        std::fs::write(holder.path().join("secret.md"), "s").unwrap();
        assert!(resolve_file(&mem, "../secret.md").is_err());
        assert!(resolve_file(&mem, "/etc/hosts").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(holder.path().join("secret.md"), mem.join("link.md"))
                .unwrap();
            assert!(resolve_file(&mem, "link.md").is_err());
        }
    }

    #[test]
    fn resolve_file_errors_name_missing_files() {
        let tmp = memory_fixture();
        let err = resolve_file(tmp.path(), "NOPE.md").unwrap_err().to_string();
        assert!(err.contains("NOPE.md"), "{err}");
    }

    #[test]
    fn editor_resolution_order_is_visual_editor_vi() {
        assert_eq!(editor_from(Some("code -w"), Some("vim")), "code -w");
        assert_eq!(editor_from(None, Some("vim")), "vim");
        assert_eq!(editor_from(Some(""), Some("vim")), "vim"); // empty = unset
        assert_eq!(editor_from(None, None), "vi");
        assert_eq!(editor_from(None, Some("")), "vi");
    }

    #[test]
    #[cfg(unix)]
    fn files_ignores_symlinked_dirs_without_looping() {
        let tmp = memory_fixture(); // has skills/research/SKILL.md
                                    // A symlink under skills/ pointing back at the memory root would loop
                                    // a symlink-following walk forever.
        std::os::unix::fs::symlink(tmp.path(), tmp.path().join("skills/loop")).unwrap();
        let files = files(tmp.path()).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                "AGENT.md",
                "USER.md",
                "MEMORY.md",
                "skills/research/SKILL.md"
            ]
        );
    }
}
