use damon_core::entity::RuntimeId;
use std::process::Command;

fn found(bin: &str, arg: &str) -> bool {
    Command::new(bin)
        .arg(arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn hint(pkg: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("install: brew install {pkg}")
    } else {
        format!("install: sudo pacman -S {pkg}")
    }
}

pub fn run() -> anyhow::Result<()> {
    let mut required_missing = Vec::new();

    let git_ok = found("git", "--version");
    println!(
        "git      {}",
        if git_ok { "ok".into() } else { hint("git") }
    );
    if !git_ok {
        required_missing.push("git");
    }

    let tmux_line = match damon_tmux::version() {
        Ok((ma, mi)) if (ma, mi) >= (3, 2) => format!("ok ({ma}.{mi})"),
        Ok((ma, mi)) => format!("too old ({ma}.{mi}, need >= 3.2) — {}", hint("tmux")),
        Err(_) => hint("tmux"),
    };
    println!("tmux     {tmux_line}");
    if !tmux_line.starts_with("ok") {
        required_missing.push("tmux");
    }

    // Ghostty: app-bundle check on macOS; PATH lookup on Linux.
    let ghostty_ok = if cfg!(target_os = "macos") {
        std::path::Path::new("/Applications/Ghostty.app").exists() || found("ghostty", "--version")
    } else {
        found("ghostty", "--version")
    };
    println!(
        "ghostty  {}",
        if ghostty_ok {
            "ok".into()
        } else {
            format!("{} (or use launcher = \"env-terminal\")", hint("ghostty"))
        }
    );

    for rt in [RuntimeId::Claude, RuntimeId::Codex, RuntimeId::Opencode] {
        let bin = rt.binary();
        let ok = found(&bin, "--version");
        println!(
            "{:<8} {}",
            rt.as_str(),
            if ok {
                "ok".to_string()
            } else {
                format!("not found ({bin}) — optional until you use it")
            }
        );
    }

    if required_missing.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("required tools missing: {}", required_missing.join(", "))
    }
}
