use crate::CoreError;
use std::path::Path;

pub const AGENT_MD: &str = "# {name}\n\n{role}\n\nThis file is your identity and operating brief. Keep it short. Refine it\nas you learn who you are and how your human wants you to operate.\n";

pub const USER_MD: &str = "# User profile\n\nWhat you know about your human: name, preferences, communication style,\nhard rules. Start empty; fill it in as you learn. Consolidate, don't append.\n";

pub const MEMORY_MD: &str = "# Memory\n\nYour notes: project conventions, tool quirks, lessons learned. Keep an\nindex here; put long topics in their own files next to this one.\n\n## Write-back protocol\n\n- Save: stated preferences, corrections, durable facts, confirmed approaches.\n- Skip: trivia, one-off state, anything easily re-discovered.\n- Consolidate rather than endlessly append; delete notes that turn out wrong.\n- At session end, review the conversation and update memory and skills.\n";

pub fn scaffold_memory(dir: &Path, name: &str, role: Option<&str>) -> Result<(), CoreError> {
    let io = |p: &Path, e: std::io::Error| CoreError::Io { path: p.to_path_buf(), source: e };
    let skills = dir.join("skills");
    std::fs::create_dir_all(&skills).map_err(|e| io(&skills, e))?;

    // Single-pass render: replace {role} in template segments only, then join with name
    let role_text = role.unwrap_or("Role: to be shaped in conversation.");
    let agent_md: String = AGENT_MD
        .split("{name}")
        .map(|seg| seg.replace("{role}", role_text))
        .collect::<Vec<_>>()
        .join(name);

    let files = [
        ("AGENT.md", agent_md),
        ("USER.md", USER_MD.to_string()),
        ("MEMORY.md", MEMORY_MD.to_string()),
    ];
    for (file, content) in files {
        let path = dir.join(file);
        if !path.exists() {
            std::fs::write(&path, content).map_err(|e| io(&path, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffolds_all_memory_files() {
        let tmp = tempfile::tempdir().unwrap();
        scaffold_memory(tmp.path(), "Scout", Some("Researches topics")).unwrap();
        let agent = std::fs::read_to_string(tmp.path().join("AGENT.md")).unwrap();
        assert!(agent.contains("# Scout"));
        assert!(agent.contains("Researches topics"));
        assert!(tmp.path().join("USER.md").exists());
        let memory = std::fs::read_to_string(tmp.path().join("MEMORY.md")).unwrap();
        assert!(memory.contains("Write-back protocol"));
        assert!(tmp.path().join("skills").is_dir());
    }

    #[test]
    fn never_overwrites_existing_memory() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("AGENT.md"), "precious").unwrap();
        scaffold_memory(tmp.path(), "Scout", None).unwrap();
        assert_eq!(std::fs::read_to_string(tmp.path().join("AGENT.md")).unwrap(), "precious");
    }

    #[test]
    fn role_none_seeds_placeholder() {
        let tmp = tempfile::tempdir().unwrap();
        scaffold_memory(tmp.path(), "Scout", None).unwrap();
        let agent = std::fs::read_to_string(tmp.path().join("AGENT.md")).unwrap();
        assert!(agent.contains("Role: to be shaped in conversation."));
    }

    #[test]
    fn braces_in_name_and_role_render_literally() {
        let tmp = tempfile::tempdir().unwrap();
        scaffold_memory(tmp.path(), "Bob{role}", Some("Helps with {name} things")).unwrap();
        let agent = std::fs::read_to_string(tmp.path().join("AGENT.md")).unwrap();
        assert!(agent.contains("# Bob{role}"));
        assert!(agent.contains("Helps with {name} things"));
    }
}
