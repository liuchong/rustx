use std::env;
use std::process::{exit, Command};

fn main() {
    let mut args = env::args();

    let rustx = args.next().unwrap_or_else(|| "rustx".into());

    let script = args.next().unwrap_or_else(|| {
        eprintln!("Usage: {} script.rs", rustx);
        exit(1);
    });

    let mut cmd = Command::new("cargo");
    cmd.arg("script").arg(script).arg("--");
    for arg in args {
        cmd.arg(arg);
    }

    let status_code = match cmd.status() {
        Ok(st) => st.code(),
        Err(_) => None,
    };
    let exit_status = match status_code {
        Some(c) => c,
        None => !0,
    };

    exit(exit_status);
}
