//! Guards against KNOWN_PATTERNS in cortado-git drifting from the filenames
//! cortado-core actually writes. If a runtime starts emitting a new bridge
//! file, this fails until cortado-git's KNOWN_PATTERNS is updated.
use cortado_core::bridge::write_bridges;
use cortado_core::entity::RuntimeId;
use std::collections::BTreeSet;

#[test]
fn known_patterns_cover_every_bridge_filename() {
    let mut produced: BTreeSet<String> = BTreeSet::new();
    for rt in [RuntimeId::Claude, RuntimeId::Codex, RuntimeId::Opencode] {
        let tmp = tempfile::tempdir().unwrap();
        let memory = tmp.path().join("memory"); // tempdir path has no whitespace
        std::fs::create_dir_all(&memory).unwrap();
        for f in ["AGENT.md", "USER.md", "MEMORY.md"] {
            std::fs::write(memory.join(f), "x").unwrap();
        }
        let worktree = tmp.path().join("worktree");
        std::fs::create_dir_all(&worktree).unwrap();
        // cortado_exe with no whitespace so the Claude settings.json is written.
        let out = write_bridges(rt, "Scout", &memory, &worktree, "cortado").unwrap();
        for p in out.written {
            let rel = p
                .strip_prefix(&worktree)
                .expect("bridge written under worktree")
                .to_string_lossy()
                .into_owned();
            produced.insert(rel);
        }
    }
    let known: BTreeSet<String> = cortado_git::known_patterns()
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        produced, known,
        "write_bridges filenames drifted from cortado-git KNOWN_PATTERNS"
    );
}
