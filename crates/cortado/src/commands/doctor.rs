use cortado_core::entity::RuntimeId;
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

const REQUIRED: [&str; 2] = ["git", "herdr"];

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

fn check_herdr() -> CheckResult {
    let version_out = Command::new("herdr").arg("--version").output();
    let Ok(out) = version_out else {
        return CheckResult {
            name: "herdr",
            status: CheckStatus::Missing,
            hint: Some(hint("herdr")),
        };
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    match cortado_herdr::parse_herdr_version(&text) {
        Some((ma, mi)) if (ma, mi) >= (0, 7) => {
            let server = Command::new("herdr")
                .args(["status", "server"])
                .output()
                .map(|o| cortado_herdr::parse_status_running(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or(false);
            let detail = if server {
                format!("({ma}.{mi}, server running)")
            } else {
                format!("({ma}.{mi}, server not running — starts on `cortado open`)")
            };
            CheckResult {
                name: "herdr",
                status: CheckStatus::Ok(detail),
                hint: None,
            }
        }
        Some(found) => CheckResult {
            name: "herdr",
            status: CheckStatus::TooOld {
                found,
                need: (0, 7),
            },
            hint: Some(hint("herdr")),
        },
        None => CheckResult {
            name: "herdr",
            status: CheckStatus::Missing,
            hint: Some(hint("herdr")),
        },
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
    let mut checks = vec![check_git(), check_herdr()];
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
        let checks = vec![ok("git"), ok("herdr")];
        assert!(failed_required(&checks).is_empty());
    }

    #[test]
    fn gate_reads_status_not_rendered_text() {
        let checks = vec![
            ok("git"),
            CheckResult {
                name: "herdr",
                status: CheckStatus::TooOld {
                    found: (0, 6),
                    need: (0, 7),
                },
                hint: Some("install: brew install herdr".into()),
            },
            // Optional tools never gate, whatever their status.
            CheckResult {
                name: "claude",
                status: CheckStatus::Missing,
                hint: Some("not found (claude) — optional until you use it".into()),
            },
        ];
        assert_eq!(failed_required(&checks), vec!["herdr"]);
    }

    #[test]
    fn render_matches_legacy_output() {
        assert_eq!(render(&ok("git")), "ok");
        assert_eq!(
            render(&CheckResult {
                name: "herdr",
                status: CheckStatus::Ok("(0.7, server running)".into()),
                hint: None,
            }),
            "ok (0.7, server running)"
        );
        assert_eq!(
            render(&CheckResult {
                name: "herdr",
                status: CheckStatus::TooOld {
                    found: (0, 6),
                    need: (0, 7),
                },
                hint: Some("install: brew install herdr".into()),
            }),
            "too old (0.6, need >= 0.7) — install: brew install herdr"
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
