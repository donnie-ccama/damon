use std::io::Read;

/// Claude Code Stop hook: on the first pass (`stop_hook_active` false/absent),
/// block the stop and instruct the agent to write back to its canonical
/// memory. On the second pass (Claude Code re-invokes the hook after the
/// agent responds to the blocked stop), `stop_hook_active` is true and we
/// allow the session to end.
///
/// Malformed or unreadable stdin is treated the same as a first pass — we
/// fail toward asking the agent to reflect once, never toward crashing or
/// silently allowing the stop.
pub fn reflect() -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok();
    let v: serde_json::Value = serde_json::from_str(&input).unwrap_or(serde_json::Value::Null);
    if v.get("stop_hook_active")
        .and_then(|b| b.as_bool())
        .unwrap_or(false)
    {
        return Ok(()); // second pass: reflection done, allow the stop
    }
    eprintln!(
        "Before finishing: review this session against the write-back protocol in \
         MEMORY.md. Update AGENT.md, USER.md, MEMORY.md and skills/ in your canonical \
         memory directory with anything durable you learned, then finish."
    );
    std::process::exit(2);
}
