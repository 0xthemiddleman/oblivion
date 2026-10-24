use oblivion_transfer::channel::Channel;
use oblivion_transfer::endpoint;
use std::path::Path;
use std::time::Duration;
mod runtime;

fn main() {
    if execute().is_err() {
        eprintln!("transfer_failed");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), Box<dyn std::error::Error>> {
    runtime::harden()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3
        || !matches!(args[0].as_str(), "send" | "receive")
        || !Path::new(&args[1]).is_absolute()
    {
        return Err("invalid_arguments".into());
    }
    let budget: u64 = args[2].parse()?;
    if !(1..=3_600_000).contains(&budget) {
        return Err("invalid_deadline".into());
    }
    // The launcher hands off an already authorized descriptor, never a socket
    // path or port supplied by the peer. Selected names stay off argv/environment.
    let mut channel = Channel::new(runtime::inherited(3)?, Duration::from_millis(budget))?;
    if args[0] == "send" {
        endpoint::send(&mut channel, Path::new(&args[1]))?;
    }
    if args[0] == "receive" {
        endpoint::accept(&mut channel, Path::new(&args[1]))?;
    }
    Ok(())
}
