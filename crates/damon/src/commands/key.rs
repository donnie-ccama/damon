use std::io::{IsTerminal, Read};

pub fn set(provider: &str) -> anyhow::Result<()> {
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
    keyring::Entry::new("damon", provider)?.delete_password()?;
    println!("removed key for {provider}");
    Ok(())
}
