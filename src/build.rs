use crate::cache::{self, CacheEntry};
use crate::cli::{Profile, RunConfig};
use crate::process;

use std::process::Command;

pub fn build(entry: &CacheEntry, config: &RunConfig) -> Result<i32, String> {
    let mut command = Command::new(cache::cargo_program());
    command.arg("build");

    if config.profile == Profile::Release {
        command.arg("--release");
    }

    command.arg("--manifest-path").arg(&entry.manifest_path);

    if let Some(target) = &config.build_flags.target {
        command.arg("--target").arg(target);
    }
    if let Some(features) = &config.build_flags.features {
        command.arg("--features").arg(features);
    }
    if config.build_flags.all_features {
        command.arg("--all-features");
    }
    if config.build_flags.no_default_features {
        command.arg("--no-default-features");
    }
    if config.build_flags.offline {
        command.arg("--offline");
    }
    if config.build_flags.locked {
        command.arg("--locked");
    }
    if config.build_flags.frozen {
        command.arg("--frozen");
    }
    if config.build_flags.quiet {
        command.arg("--quiet");
    }
    if config.build_flags.verbose {
        command.arg("--verbose");
    }

    let status = command.status().map_err(|err| {
        format!(
            "failed to execute cargo build for {}: {}",
            entry.manifest_path.display(),
            err
        )
    })?;

    Ok(process::exit_code(status))
}
