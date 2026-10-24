use oblivion_transport::Transport;
use std::io::{self, Write};
use std::process::{Command, Stdio};

// The immutable ruleset starts with empty selectors. Only this trusted process
// may select an egress route; every update is one atomic nftables transaction.
pub fn configure(transport: &Transport) -> io::Result<()> {
    transport.validate().map_err(io::Error::other)?;
    let mut policy = String::from(
        "flush set inet oblivion proxy4\nflush set inet oblivion tor_owners\nflush set inet oblivion tor_connectors\n",
    );
    match transport {
        Transport::Tor {} => policy.push_str(
            "add element inet oblivion tor_owners { 1200 }\nadd element inet oblivion tor_connectors { 1201 }\n",
        ),
        Transport::Socks5 { endpoint, .. } => policy.push_str(&format!(
            "add element inet oblivion proxy4 {{ {} . {} }}\n",
            endpoint.ip(), endpoint.port(),
        )),
    }
    let mut child = Command::new("/usr/sbin/nft")
        .args(["-f", "-"])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let written = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing_policy_input"))?
        .write_all(policy.as_bytes());
    let status = child.wait()?;
    written?;
    if !status.success() {
        return Err(io::Error::other("network_policy_failed"));
    }
    Ok(())
}
