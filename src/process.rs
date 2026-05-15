use std::process::ExitStatus;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

pub fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }

    #[cfg(unix)]
    if let Some(signal) = status.signal() {
        return 128 + signal;
    }

    1
}
