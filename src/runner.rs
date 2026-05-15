use crate::build;
use crate::cache::{self, CacheEntry, CacheKeyInput};
use crate::cli::{self, Action, RunConfig};
use crate::manifest::{self, GeneratedManifest};
use crate::process;
use crate::script::{self, ScriptInput};

use std::process::Command;

pub fn run<I>(args: I) -> i32
where
    I: IntoIterator<Item = String>,
{
    match try_run(args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("rustx: {}", err);
            1
        }
    }
}

fn try_run<I>(args: I) -> Result<i32, String>
where
    I: IntoIterator<Item = String>,
{
    match cli::parse(args)? {
        Action::Help { program } => {
            println!("{}", cli::help(&program));
            Ok(0)
        }
        Action::Version => {
            println!("rustx {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Action::ClearCache => {
            let root = cache::cache_root()?;
            cache::clear(&root)?;
            Ok(0)
        }
        Action::PrintCacheDir => {
            println!("{}", cache::cache_root()?.display());
            Ok(0)
        }
        Action::Run(config) => run_script(&config),
    }
}

fn run_script(config: &RunConfig) -> Result<i32, String> {
    let script = script::load(&config.script)?;
    let default_package_name =
        cache::generated_package_name(&script.absolute_path, &script.raw_contents);
    let generated_manifest =
        manifest::generate(script.manifest_block.as_deref(), &default_package_name)?;
    let root = cache::cache_root()?;
    let key = cache::key(cache_key_input(&script, &generated_manifest, config));
    let entry = cache::entry(
        &root,
        &key,
        &generated_manifest.package_name,
        config.profile,
        config.build_flags.target.as_deref(),
    );
    let expected_metadata = cache::metadata(
        cache_key_input(&script, &generated_manifest, config),
        &key,
        &entry.binary_path,
    );

    if config.print_generated {
        cache::write_generated_package(&entry, &generated_manifest.contents, &script.rust_source)?;
        println!("{}", entry.dir.display());
        return Ok(0);
    }

    if config.force || !cache::is_fresh(&entry, &expected_metadata) {
        if let Some(code) = build_script(
            &entry,
            config,
            &generated_manifest,
            &script,
            &expected_metadata,
        )? {
            return Ok(code);
        }
    }

    execute_script_binary(&entry, config)
}

fn build_script(
    entry: &CacheEntry,
    config: &RunConfig,
    generated_manifest: &GeneratedManifest,
    script: &ScriptInput,
    expected_metadata: &str,
) -> Result<Option<i32>, String> {
    cache::write_generated_package(entry, &generated_manifest.contents, &script.rust_source)?;
    let build_status = build::build(entry, config)?;

    if build_status != 0 {
        return Ok(Some(build_status));
    }

    if !entry.binary_path.is_file() {
        return Err(format!(
            "cached binary was not produced: {}",
            entry.binary_path.display()
        ));
    }

    cache::write_metadata(entry, expected_metadata)?;
    Ok(None)
}

fn execute_script_binary(entry: &CacheEntry, config: &RunConfig) -> Result<i32, String> {
    let mut command = Command::new(&entry.binary_path);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.arg0(&config.script_display);
    }

    command.args(&config.script_args);

    let status = command.status().map_err(|err| {
        format!(
            "failed to execute cached binary {}: {}",
            entry.binary_path.display(),
            err
        )
    })?;

    Ok(process::exit_code(status))
}

fn cache_key_input<'a>(
    script: &'a ScriptInput,
    generated_manifest: &'a GeneratedManifest,
    config: &'a RunConfig,
) -> CacheKeyInput<'a> {
    CacheKeyInput {
        absolute_script_path: &script.absolute_path,
        raw_script: &script.raw_contents,
        generated_manifest: &generated_manifest.contents,
        rust_source: &script.rust_source,
        package_name: &generated_manifest.package_name,
        profile: config.profile,
        build_flags: &config.build_flags,
    }
}
