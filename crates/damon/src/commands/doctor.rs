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

#[derive(Debug, PartialEq)]
enum CheckStatus {
    /// Detail text appended after "ok" (empty renders as plain "ok").
    Ok(String),
    Missing,
    TooOld {
        found: (u32, u32),
        need: (u32, u32),
    },
}

#[derive(Debug)]
struct CheckResult {
    name: &'static str,
    status: CheckStatus,
    hint: Option<String>,
}

impl CheckResult {
    fn passed(&self) -> bool {
        matches!(self.status, CheckStatus::Ok(_))
    }
}

/// Display line for one check. Gating never reads this — see failed_required.
fn render(c: &CheckResult) -> String {
    match &c.status {
        CheckStatus::Ok(detail) if detail.is_empty() => "ok".to_string(),
        CheckStatus::Ok(detail) => format!("ok {detail}"),
        CheckStatus::Missing => c.hint.clone().unwrap_or_else(|| "missing".to_string()),
        CheckStatus::TooOld { found, need } => format!(
            "too old ({}.{}, need >= {}.{}){}",
            found.0,
            found.1,
            need.0,
            need.1,
            c.hint
                .as_ref()
                .map(|h| format!(" — {h}"))
                .unwrap_or_default()
        ),
    }
}

const REQUIRED: [&str; 2] = ["git", "tmux"];

fn failed_required(checks: &[CheckResult]) -> Vec<&'static str> {
    checks
        .iter()
        .filter(|c| REQUIRED.contains(&c.name) && !c.passed())
        .map(|c| c.name)
        .collect()
}

fn check_git() -> CheckResult {
    if found("git", "--version") {
        CheckResult {
            name: "git",
            status: CheckStatus::Ok(String::new()),
            hint: None,
        }
    } else {
        CheckResult {
            name: "git",
            status: CheckStatus::Missing,
            hint: Some(hint("git")),
        }
    }
}

fn check_tmux() -> CheckResult {
    match damon_tmux::version() {
        Ok((ma, mi)) if (ma, mi) >= (3, 2) => CheckResult {
            name: "tmux",
            status: CheckStatus::Ok(format!("({ma}.{mi})")),
            hint: None,
        },
        Ok((ma, mi)) => CheckResult {
            name: "tmux",
            status: CheckStatus::TooOld {
                found: (ma, mi),
                need: (3, 2),
            },
            hint: Some(hint("tmux")),
        },
        Err(_) => CheckResult {
            name: "tmux",
            status: CheckStatus::Missing,
            hint: Some(hint("tmux")),
        },
    }
}

fn check_ghostty() -> CheckResult {
    // App-bundle check on macOS; PATH lookup on Linux.
    let ok = if cfg!(target_os = "macos") {
        std::path::Path::new("/Applications/Ghostty.app").exists() || found("ghostty", "--version")
    } else {
        found("ghostty", "--version")
    };
    if ok {
        CheckResult {
            name: "ghostty",
            status: CheckStatus::Ok(String::new()),
            hint: None,
        }
    } else {
        CheckResult {
            name: "ghostty",
            status: CheckStatus::Missing,
            hint: Some(format!(
                "{} (or use launcher = \"env-terminal\")",
                hint("ghostty")
            )),
        }
    }
}

fn check_runtime(rt: RuntimeId) -> CheckResult {
    let bin = rt.binary();
    if found(&bin, "--version") {
        CheckResult {
            name: rt.as_str(),
            status: CheckStatus::Ok(String::new()),
            hint: None,
        }
    } else {
        CheckResult {
            name: rt.as_str(),
            status: CheckStatus::Missing,
            hint: Some(format!("not found ({bin}) — optional until you use it")),
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let mut checks = vec![check_git(), check_tmux(), check_ghostty()];
    for rt in [RuntimeId::Claude, RuntimeId::Codex, RuntimeId::Opencode] {
        checks.push(check_runtime(rt));
    }
    for c in &checks {
        println!("{:<8} {}", c.name, render(c));
    }
    let missing = failed_required(&checks);
    if missing.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("required tools missing: {}", missing.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(name: &'static str) -> CheckResult {
        CheckResult {
            name,
            status: CheckStatus::Ok(String::new()),
            hint: None,
        }
    }

    #[test]
    fn gate_passes_when_required_checks_are_ok() {
        let checks = vec![ok("git"), ok("tmux")];
        assert!(failed_required(&checks).is_empty());
    }

    #[test]
    fn gate_reads_status_not_rendered_text() {
        let checks = vec![
            ok("git"),
            CheckResult {
                name: "tmux",
                status: CheckStatus::TooOld {
                    found: (3, 1),
                    need: (3, 2),
                },
                hint: Some("install: brew install tmux".into()),
            },
            // Optional tools never gate, whatever their status.
            CheckResult {
                name: "claude",
                status: CheckStatus::Missing,
                hint: Some("not found (claude) — optional until you use it".into()),
            },
        ];
        assert_eq!(failed_required(&checks), vec!["tmux"]);
    }

    #[test]
    fn render_matches_legacy_output() {
        assert_eq!(render(&ok("git")), "ok");
        assert_eq!(
            render(&CheckResult {
                name: "tmux",
                status: CheckStatus::Ok("(3.7)".into()),
                hint: None,
            }),
            "ok (3.7)"
        );
        assert_eq!(
            render(&CheckResult {
                name: "tmux",
                status: CheckStatus::TooOld {
                    found: (3, 1),
                    need: (3, 2),
                },
                hint: Some("install: brew install tmux".into()),
            }),
            "too old (3.1, need >= 3.2) — install: brew install tmux"
        );
        assert_eq!(
            render(&CheckResult {
                name: "git",
                status: CheckStatus::Missing,
                hint: Some("install: brew install git".into()),
            }),
            "install: brew install git"
        );
    }
}
