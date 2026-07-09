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
        let path = entry
            .map_err(|e| anyhow::anyhow!("{}: {e}", dir.display()))?
            .path();
        if path.is_dir() {
            collect_files(&path, base, out)?;
        } else if path.is_file() {
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
    if edit {
        anyhow::bail!("--edit lands in the next task"); // Task 10 replaces this line
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
}
