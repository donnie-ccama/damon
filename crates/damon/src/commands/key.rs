use std::io::{IsTerminal, Read};

pub fn set(provider: &str) -> anyhow::Result<()> {
    if std::env::var("DAMON_NO_KEYRING").is_ok_and(|v| !v.is_empty()) {
        anyhow::bail!("OS keyring disabled (DAMON_NO_KEYRING is set)");
    }
    let key = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("key for {provider} (input hidden): "))?
    } else {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    };
    let key = key.trim();
    if key.is_empty() {
        anyhow::bail!("key must not be empty");
    }
    keyring::Entry::new("damon", provider)?.set_password(key)?;
    println!("stored key for {provider} (OS keyring, service \"damon\")");
    Ok(())
}

pub fn rm(provider: &str) -> anyhow::Result<()> {
    if std::env::var("DAMON_NO_KEYRING").is_ok_and(|v| !v.is_empty()) {
        anyhow::bail!("OS keyring disabled (DAMON_NO_KEYRING is set)");
    }
    let entry = keyring::Entry::new("damon", provider)?;
    entry.delete_password().map_err(|e| match e {
        keyring::Error::NoEntry => anyhow::anyhow!("no key stored for {provider}"),
        other => anyhow::anyhow!("keyring error for {provider}: {other}"),
    })?;
    println!("removed key for {provider}");
    Ok(())
}
