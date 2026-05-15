use crate::cli::{BuildFlags, Profile};

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub const CACHE_FORMAT_VERSION: &str = "1";

#[derive(Debug)]
pub struct CacheEntry {
    pub dir: PathBuf,
    pub manifest_path: PathBuf,
    pub source_path: PathBuf,
    pub meta_path: PathBuf,
    pub binary_path: PathBuf,
}

pub struct CacheKeyInput<'a> {
    pub absolute_script_path: &'a Path,
    pub raw_script: &'a str,
    pub generated_manifest: &'a str,
    pub rust_source: &'a str,
    pub package_name: &'a str,
    pub profile: Profile,
    pub build_flags: &'a BuildFlags,
}

pub fn cache_root() -> Result<PathBuf, String> {
    if let Some(path) = non_empty_env_path("RUSTX_CACHE_DIR") {
        return Ok(path);
    }

    if let Some(cargo_home) = non_empty_env_path("CARGO_HOME") {
        return Ok(cargo_home.join("rustx").join("cache"));
    }

    if let Some(home) = non_empty_env_path("HOME") {
        return Ok(home.join(".cargo").join("rustx").join("cache"));
    }

    Err("cannot determine cache directory; set RUSTX_CACHE_DIR or HOME".to_string())
}

pub fn clear(root: &Path) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    reject_dangerous_cache_root(root)?;
    fs::remove_dir_all(root).map_err(|err| {
        format!(
            "failed to remove cache directory {}: {}",
            root.display(),
            err
        )
    })
}

pub fn generated_package_name(absolute_script_path: &Path, raw_script: &str) -> String {
    let path = absolute_script_path.to_string_lossy();
    let hash = stable_hash_hex(&[path.as_bytes(), raw_script.as_bytes()]);
    format!("rustx_script_{}", &hash[..16])
}

pub fn key(input: CacheKeyInput<'_>) -> String {
    let path = input.absolute_script_path.to_string_lossy();
    let target = input.build_flags.target.as_deref().unwrap_or("");
    let toolchain = env::var("RUSTUP_TOOLCHAIN").unwrap_or_default();
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let cargo = cargo_program();
    let cargo = cargo.to_string_lossy();
    let profile = input.profile.name();
    let build_flags = input.build_flags.cache_material();

    let hash = stable_hash_hex(&[
        CACHE_FORMAT_VERSION.as_bytes(),
        path.as_bytes(),
        input.raw_script.as_bytes(),
        input.generated_manifest.as_bytes(),
        input.rust_source.as_bytes(),
        input.package_name.as_bytes(),
        profile.as_bytes(),
        target.as_bytes(),
        build_flags.as_bytes(),
        toolchain.as_bytes(),
        rustc.as_bytes(),
        cargo.as_bytes(),
    ]);

    format!("v{}-{}", CACHE_FORMAT_VERSION, hash)
}

pub fn entry(
    root: &Path,
    key: &str,
    package_name: &str,
    profile: Profile,
    target: Option<&str>,
) -> CacheEntry {
    let dir = root.join(key);
    let manifest_path = dir.join("Cargo.toml");
    let source_path = dir.join("src").join("main.rs");
    let meta_path = dir.join("rustx-meta.txt");
    let mut binary_path = dir.join("target");

    if let Some(target) = target {
        binary_path.push(target);
    }

    binary_path.push(profile.target_dir_name());
    binary_path.push(binary_file_name(package_name));

    CacheEntry {
        dir,
        manifest_path,
        source_path,
        meta_path,
        binary_path,
    }
}

pub fn metadata(input: CacheKeyInput<'_>, key: &str, binary_path: &Path) -> String {
    let path = input.absolute_script_path.to_string_lossy();
    let target = input.build_flags.target.as_deref().unwrap_or("");
    let toolchain = env::var("RUSTUP_TOOLCHAIN").unwrap_or_default();
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let cargo = cargo_program();
    let cargo = cargo.to_string_lossy();

    format!(
        "format-version={}\ncache-key={}\nscript-path={}\nscript-hash={}\nmanifest-hash={}\nsource-hash={}\npackage-name={}\nprofile={}\ntarget={}\nbuild-flags-hash={}\nrustup-toolchain={}\nrustc={}\ncargo={}\nbinary-path={}\n",
        CACHE_FORMAT_VERSION,
        key,
        path,
        stable_hash_hex(&[input.raw_script.as_bytes()]),
        stable_hash_hex(&[input.generated_manifest.as_bytes()]),
        stable_hash_hex(&[input.rust_source.as_bytes()]),
        input.package_name,
        input.profile.name(),
        target,
        stable_hash_hex(&[input.build_flags.cache_material().as_bytes()]),
        toolchain,
        rustc,
        cargo,
        binary_path.display()
    )
}

pub fn is_fresh(entry: &CacheEntry, expected_metadata: &str) -> bool {
    if !entry.binary_path.is_file() {
        return false;
    }

    match fs::read_to_string(&entry.meta_path) {
        Ok(existing) => existing == expected_metadata,
        Err(_) => false,
    }
}

pub fn write_generated_package(
    entry: &CacheEntry,
    manifest: &str,
    source: &str,
) -> Result<(), String> {
    fs::create_dir_all(entry.source_path.parent().expect("source path has parent")).map_err(
        |err| {
            format!(
                "failed to create cache directory {}: {}",
                entry.dir.display(),
                err
            )
        },
    )?;
    write_if_changed(&entry.manifest_path, manifest)?;
    write_if_changed(&entry.source_path, source)
}

pub fn write_metadata(entry: &CacheEntry, metadata: &str) -> Result<(), String> {
    write_if_changed(&entry.meta_path, metadata)
}

pub fn cargo_program() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

pub fn stable_hash_hex(parts: &[&[u8]]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;

    for part in parts {
        for byte in part.len().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    format!("{:016x}", hash)
}

fn binary_file_name(package_name: &str) -> String {
    format!("{}{}", package_name, env::consts::EXE_SUFFIX)
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    if fs::read_to_string(path)
        .map(|existing| existing == contents)
        .unwrap_or(false)
    {
        return Ok(());
    }

    fs::write(path, contents).map_err(|err| format!("failed to write {}: {}", path.display(), err))
}

fn non_empty_env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
}

fn reject_dangerous_cache_root(root: &Path) -> Result<(), String> {
    let canonical = root.canonicalize().map_err(|err| {
        format!(
            "failed to inspect cache directory {}: {}",
            root.display(),
            err
        )
    })?;

    if canonical.parent().is_none() || canonical.file_name().is_none() {
        return Err(format!(
            "refusing to clear dangerous cache directory: {}",
            canonical.display()
        ));
    }

    if let Some(home) = non_empty_env_path("HOME").and_then(|path| path.canonicalize().ok()) {
        if canonical == home {
            return Err(format!(
                "refusing to clear home directory as cache: {}",
                canonical.display()
            ));
        }
    }

    if let Some(cargo_home) =
        non_empty_env_path("CARGO_HOME").and_then(|path| path.canonicalize().ok())
    {
        if canonical == cargo_home {
            return Err(format!(
                "refusing to clear Cargo home as cache: {}",
                canonical.display()
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{generated_package_name, key, stable_hash_hex, CacheKeyInput};
    use crate::cli::{BuildFlags, Profile};
    use std::path::Path;

    #[test]
    fn hash_is_stable() {
        assert_eq!(stable_hash_hex(&[b"abc"]), stable_hash_hex(&[b"abc"]));
        assert_ne!(stable_hash_hex(&[b"abc"]), stable_hash_hex(&[b"abcd"]));
    }

    #[test]
    fn generated_package_name_is_valid_shape() {
        let name = generated_package_name(Path::new("/tmp/demo.rs"), "fn main() {}");

        assert!(name.starts_with("rustx_script_"));
        assert!(name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'));
    }

    #[test]
    fn cache_key_changes_when_script_changes() {
        let flags = BuildFlags::default();
        let first = key(CacheKeyInput {
            absolute_script_path: Path::new("/tmp/demo.rs"),
            raw_script: "fn main() { println!(\"one\"); }",
            generated_manifest: "[package]\nname = \"rustx_script_demo\"\n",
            rust_source: "fn main() { println!(\"one\"); }",
            package_name: "rustx_script_demo",
            profile: Profile::Debug,
            build_flags: &flags,
        });
        let second = key(CacheKeyInput {
            absolute_script_path: Path::new("/tmp/demo.rs"),
            raw_script: "fn main() { println!(\"two\"); }",
            generated_manifest: "[package]\nname = \"rustx_script_demo\"\n",
            rust_source: "fn main() { println!(\"two\"); }",
            package_name: "rustx_script_demo",
            profile: Profile::Debug,
            build_flags: &flags,
        });

        assert_ne!(first, second);
    }

    #[test]
    fn cache_key_changes_when_profile_changes() {
        let flags = BuildFlags::default();
        let debug = key(CacheKeyInput {
            absolute_script_path: Path::new("/tmp/demo.rs"),
            raw_script: "fn main() {}",
            generated_manifest: "[package]\nname = \"rustx_script_demo\"\n",
            rust_source: "fn main() {}",
            package_name: "rustx_script_demo",
            profile: Profile::Debug,
            build_flags: &flags,
        });
        let release = key(CacheKeyInput {
            absolute_script_path: Path::new("/tmp/demo.rs"),
            raw_script: "fn main() {}",
            generated_manifest: "[package]\nname = \"rustx_script_demo\"\n",
            rust_source: "fn main() {}",
            package_name: "rustx_script_demo",
            profile: Profile::Release,
            build_flags: &flags,
        });

        assert_ne!(debug, release);
    }
}
