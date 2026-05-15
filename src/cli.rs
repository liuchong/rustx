use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    pub fn target_dir_name(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }
}

#[derive(Debug, Default)]
pub struct BuildFlags {
    pub target: Option<String>,
    pub features: Option<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub offline: bool,
    pub locked: bool,
    pub frozen: bool,
    pub quiet: bool,
    pub verbose: bool,
}

impl BuildFlags {
    pub fn cache_material(&self) -> String {
        format!(
            "target={:?}\nfeatures={:?}\nall_features={}\nno_default_features={}\noffline={}\nlocked={}\nfrozen={}\n",
            self.target,
            self.features,
            self.all_features,
            self.no_default_features,
            self.offline,
            self.locked,
            self.frozen
        )
    }
}

#[derive(Debug)]
pub struct RunConfig {
    pub script: PathBuf,
    pub script_display: String,
    pub script_args: Vec<String>,
    pub force: bool,
    pub print_generated: bool,
    pub profile: Profile,
    pub build_flags: BuildFlags,
}

#[derive(Debug)]
pub enum Action {
    Help { program: String },
    Version,
    ClearCache,
    PrintCacheDir,
    Run(RunConfig),
}

pub fn parse<I>(args: I) -> Result<Action, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let program = args.next().unwrap_or_else(|| "rustx".to_string());

    let mut force = false;
    let mut print_generated = false;
    let mut clear_cache = false;
    let mut print_cache_dir = false;
    let mut profile = Profile::Debug;
    let mut build_flags = BuildFlags::default();
    let mut script = None;
    let mut script_args = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Action::Help { program }),
            "-V" | "--version" => return Ok(Action::Version),
            "--force" => force = true,
            "--release" => profile = Profile::Release,
            "--clear-cache" => clear_cache = true,
            "--print-cache-dir" => print_cache_dir = true,
            "--print-generated" => print_generated = true,
            "--target" => {
                build_flags.target = Some(require_value(&mut args, "--target")?);
            }
            "--features" => {
                build_flags.features = Some(require_value(&mut args, "--features")?);
            }
            "--all-features" => build_flags.all_features = true,
            "--no-default-features" => build_flags.no_default_features = true,
            "--offline" => build_flags.offline = true,
            "--locked" => build_flags.locked = true,
            "--frozen" => build_flags.frozen = true,
            "--quiet" => build_flags.quiet = true,
            "--verbose" => build_flags.verbose = true,
            "--" => {
                let script_arg = args.next().ok_or_else(|| missing_script(&program))?;
                script = Some(script_arg);
                script_args.extend(args);
                break;
            }
            other if other.starts_with('-') => {
                return Err(format!(
                    "unknown option: {}\n{}",
                    other,
                    short_usage(&program)
                ));
            }
            _ => {
                script = Some(arg);
                script_args.extend(args);
                break;
            }
        }
    }

    if clear_cache && print_cache_dir {
        return Err("--clear-cache cannot be combined with --print-cache-dir".to_string());
    }

    if clear_cache || print_cache_dir {
        if script.is_some() || force || print_generated || profile == Profile::Release {
            return Err("--clear-cache and --print-cache-dir are standalone options".to_string());
        }

        return if clear_cache {
            Ok(Action::ClearCache)
        } else {
            Ok(Action::PrintCacheDir)
        };
    }

    let script = script.ok_or_else(|| missing_script(&program))?;

    Ok(Action::Run(RunConfig {
        script: PathBuf::from(&script),
        script_display: script,
        script_args,
        force,
        print_generated,
        profile,
        build_flags,
    }))
}

pub fn short_usage(program: &str) -> String {
    format!("Usage: {} [OPTIONS] <script.rs> [--] [args...]", program)
}

pub fn help(program: &str) -> String {
    format!(
        "{usage}

Run a self-contained Rust script.

Options:
  -h, --help              Print help
  -V, --version           Print version
      --force             Rebuild the script even if a cached binary exists
      --release           Build with Cargo's release profile
      --clear-cache       Remove rustx's cache directory
      --print-cache-dir   Print rustx's cache directory
      --print-generated   Generate the cached Cargo package and print its path
      --target <triple>   Build for a Cargo target triple
      --features <list>   Build with Cargo features
      --all-features      Build with all Cargo features
      --no-default-features
                           Build without default Cargo features
      --offline           Pass --offline to Cargo while building
      --locked            Pass --locked to Cargo while building
      --frozen            Pass --frozen to Cargo while building
      --quiet             Pass --quiet to Cargo while building
      --verbose           Pass --verbose to Cargo while building",
        usage = short_usage(program)
    )
}

fn require_value<I>(args: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| format!("{} requires a value", option))
}

fn missing_script(program: &str) -> String {
    format!("missing script path\n{}", short_usage(program))
}

#[cfg(test)]
mod tests {
    use super::{parse, Action, Profile};

    #[test]
    fn script_args_start_after_script_path() {
        let action = parse([
            "rustx".to_string(),
            "--release".to_string(),
            "script.rs".to_string(),
            "--script-flag".to_string(),
        ])
        .unwrap();

        match action {
            Action::Run(config) => {
                assert_eq!(config.profile, Profile::Release);
                assert_eq!(config.script_display, "script.rs");
                assert_eq!(config.script_args, ["--script-flag"]);
            }
            _ => panic!("expected run action"),
        }
    }

    #[test]
    fn separator_allows_dash_prefixed_script_path() {
        let action = parse([
            "rustx".to_string(),
            "--".to_string(),
            "--script.rs".to_string(),
            "arg".to_string(),
        ])
        .unwrap();

        match action {
            Action::Run(config) => {
                assert_eq!(config.script_display, "--script.rs");
                assert_eq!(config.script_args, ["arg"]);
            }
            _ => panic!("expected run action"),
        }
    }

    #[test]
    fn rejects_unknown_rustx_option_before_script() {
        let error = parse(["rustx".to_string(), "--unknown".to_string()]).unwrap_err();

        assert!(error.contains("unknown option"));
    }

    #[test]
    fn parses_standalone_cache_options() {
        assert!(matches!(
            parse(["rustx".to_string(), "--clear-cache".to_string()]).unwrap(),
            Action::ClearCache
        ));
        assert!(matches!(
            parse(["rustx".to_string(), "--print-cache-dir".to_string()]).unwrap(),
            Action::PrintCacheDir
        ));
    }
}
