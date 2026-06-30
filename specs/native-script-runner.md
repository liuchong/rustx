# rustx Native Script Runner Specification

## Purpose

`rustx` should let a single Rust source file run like a shell or Python script:

```rust
#!/usr/bin/env rustx

fn main() {
    println!("hello");
}
```

The script file must be self-contained. It should not require a neighboring
`Cargo.toml`, project directory, generated files checked into source control, or
any external configuration beyond tools already expected in a Rust environment.

The original implementation delegates execution to `cargo-script`. That kept
`rustx` small, but `cargo-script` is no longer a good long-term foundation.
`rustx` should own the script-running behavior directly and should use stable
Cargo as the build backend.

## Design Principles

- A script is one file. Source code, package metadata, dependencies, and build
  configuration live in that file.
- Cached binaries are first-class. A script that has not changed should not pay
  Cargo startup and compilation cost on every run.
- `rustx` itself should avoid third-party dependencies. Parsing and cache
  management should use the Rust standard library unless a future local
  `cargo-x` crate is intentionally introduced.
- Use Cargo's package model rather than inventing a compiler pipeline.
- Prefer the direction of Cargo's official single-file package design over
  historical `cargo-script` formats.
- Keep runtime semantics script-like: current working directory is preserved,
  arguments are forwarded, and exit status is propagated.

## Background

Rust source files may begin with a shebang. The Rust Reference specifies that
the compiler removes a leading shebang line during input processing:

<https://doc.rust-lang.org/reference/input-format.html#shebang-removal>

This makes the following shape valid for executable Rust scripts:

```rust
#!/usr/bin/env rustx

fn main() {}
```

Cargo also has an unstable `-Z script` feature for single-file packages:

<https://doc.rust-lang.org/cargo/reference/unstable.html#script>

That design is the right conceptual direction, but it is nightly-only and
unstable. `rustx` should not depend on it. Instead, `rustx` should accept a
compatible embedded manifest style, generate a normal Cargo package in a cache
directory, build it with stable Cargo, then run the cached binary directly.

## Script File Format

### Basic Script

The smallest script contains only Rust code:

```rust
#!/usr/bin/env rustx

fn main() {
    println!("hello");
}
```

For scripts without an embedded manifest, `rustx` generates a default package
manifest:

```toml
[package]
name = "rustx_script"
version = "0.0.0"
edition = "2021"

[dependencies]
```

### Embedded Cargo Manifest

Scripts may include an embedded Cargo manifest block immediately after the
optional shebang and leading blank lines/comments:

```rust
#!/usr/bin/env rustx
---cargo
[package]
edition = "2021"

[dependencies]
anyhow = "1"
---

fn main() -> anyhow::Result<()> {
    println!("hello");
    Ok(())
}
```

The block starts with a line containing exactly:

```text
---cargo
```

and ends with a line containing exactly:

```text
---
```

The contents between those markers are Cargo manifest TOML. `rustx` does not
need to parse this TOML. It can copy it into the generated `Cargo.toml` and let
Cargo validate it.

### Default Package Fields

If the embedded manifest omits `[package]`, `rustx` adds a generated `[package]`
table.

If the embedded manifest includes `[package]`, `rustx` should preserve it and
only fill fields that are required for a generated binary package when they are
missing:

- `name`
- `version`
- `edition`

The generated package name must be deterministic and valid for Cargo. A good
shape is:

```text
rustx_script_<short_hash>
```

The default edition should be `2021` unless the project deliberately raises the
minimum supported Rust version later.

### Source Extraction

The generated `src/main.rs` should contain the script's Rust source after
removing:

- the leading shebang line, if present
- the embedded `---cargo ... ---` manifest block, if present

The rest of the source should be preserved byte-for-byte where practical.

### Unsupported Historical Syntax

`cargo-script` supports historical formats such as:

```rust
// cargo-deps: anyhow="1"
```

That syntax is not part of the core design. Supporting it would improve
migration from `cargo-script`, but it should be treated as an explicit
compatibility feature, not the primary script format.

## Execution Model

Given:

```text
rustx path/to/script.rs arg1 arg2
```

`rustx` performs:

1. Resolve the script path.
2. Read the script file.
3. Split the script into embedded manifest and Rust source.
4. Compute a cache key.
5. Locate or create the generated Cargo package under the cache directory.
6. If the cached binary is missing or stale, run `cargo build`.
7. Execute the cached binary directly, forwarding script arguments.
8. Exit with the child process status.

### Why Not `cargo run`

`cargo run` should not be used on cache hits. Even when nothing is recompiled,
it starts Cargo and performs freshness checks. For script usage, that overhead is
visible and frustrating.

The preferred model is:

```text
cache miss -> cargo build --manifest-path <cache>/Cargo.toml
cache hit  -> exec <cache>/target/.../<binary>
```

### Working Directory

The script process must inherit the user's current working directory. It should
not run from the generated cache package directory.

This makes file access behave like other scripting environments:

```text
cd project
./tools/report.rs data/input.json
```

The script should see `project` as its current directory.

### Arguments

All arguments after the script path are forwarded to the cached binary.

On Unix-like platforms, `rustx` should set `argv[0]` to the original script path
when possible, using `std::os::unix::process::CommandExt::arg0`. This makes
`std::env::args()` behave more like shell and Python scripts.

On platforms where setting `argv[0]` is not available, the cached binary path is
acceptable.

### Exit Status

`rustx` should propagate the script's exit behavior:

- normal exit code is returned unchanged
- signal termination should map to conventional shell status where possible
  on Unix, for example `128 + signal`
- spawn failures should print a clear `rustx:` error and exit nonzero

## Cache Design

### Cache Location

Use Cargo-oriented cache locations:

1. `$RUSTX_CACHE_DIR`, if set
2. `$CARGO_HOME/rustx/cache`, if `CARGO_HOME` is set
3. `$HOME/.cargo/rustx/cache`

The first option is useful for tests and CI. The second and third keep the
cache near the Rust toolchain rather than creating a new top-level convention.

### Cache Directory Shape

Each unique script build gets a directory:

```text
<cache-root>/<cache-key>/
  Cargo.toml
  src/
    main.rs
  target/
  rustx-meta.txt
```

The generated binary path is determined from the generated package name and
Cargo target directory.

### Cache Key

The cache key must be stable and deterministic across runs. It should include:

- normalized absolute script path
- full script file contents
- rustx cache format version
- relevant Cargo/rustc identity
- selected build profile
- selected target triple, if specified
- environment-affecting rustx options

The script path should be included even when contents match, so two different
scripts with the same source do not unexpectedly share package metadata,
diagnostic paths, or future per-script state.

`rustx` should use a small stable hash implemented in the project, such as
FNV-1a 64-bit or another simple non-cryptographic hash. The cache key is for
identity and path length control, not for security.

### Build Freshness

The fastest normal path should be:

```text
if cached binary exists and metadata matches:
    execute binary directly
else:
    regenerate package files
    cargo build
    execute binary
```

`rustx` should avoid invoking Cargo on cache hits.

The metadata file should record enough information to verify that the cache
directory matches the current invocation:

```text
format-version = 1
script-path = ...
script-hash = ...
manifest-hash = ...
source-hash = ...
package-name = ...
profile = debug
target = ...
rustc-version = ...
cargo-version = ...
binary-path = ...
```

This can be a simple line-based format to avoid adding `serde`.

### Build Profiles

Default script builds should use Cargo's dev profile for fast initial
compilation:

```text
cargo build --manifest-path <cache>/Cargo.toml
```

`rustx` should support a release option:

```text
rustx --release script.rs
```

Release and debug builds must use different cache keys or distinct target
paths.

### Cache Invalidation

The cache must rebuild when any of these change:

- script contents
- embedded manifest contents
- rustx cache format version
- build profile
- target triple
- relevant toolchain identity

Manual controls:

```text
rustx --force script.rs
rustx --clear-cache
```

`--force` rebuilds the selected script even if the binary exists.

`--clear-cache` removes the rustx cache root. This command should be explicit
and should not affect Cargo's global registry or unrelated build caches.

### Cache Cleanup

Long-term, `rustx` should provide bounded cache cleanup:

```text
rustx --cache-status
rustx --gc
rustx --gc --max-age 30d
rustx --gc --max-size 5G
```

The initial implementation should at least keep metadata structured enough to
make this possible later.

## Command Line Interface

### Primary Form

```text
rustx [OPTIONS] <script.rs> [--] [script-args...]
```

The `--` separator is optional. Everything after the script path belongs to the
script unless it is parsed as a rustx option before the script.

Examples:

```text
rustx hello.rs
rustx hello.rs one two
rustx --release hello.rs
rustx --force hello.rs
rustx hello.rs -- --script-flag
```

### Required Options

```text
--help
--version
--force
--release
--clear-cache
--print-cache-dir
--print-generated
```

`--print-generated` should print the generated package directory for debugging.
It should not execute the script.

### Future Options

```text
--target <triple>
--features <features>
--no-default-features
--all-features
--profile <name>
--offline
--locked
--frozen
--verbose
--quiet
```

These correspond to stable Cargo concepts and can be passed through during
build.

## Cargo Integration

### Build Command

Debug build:

```text
cargo build --manifest-path <cache>/Cargo.toml
```

Release build:

```text
cargo build --release --manifest-path <cache>/Cargo.toml
```

When supported options are present, append the corresponding Cargo flags.

### Target Directory

By default, use the generated package's own `target/` directory inside the
cache entry. This isolates script builds and makes cache deletion simple.

Do not write generated packages into the user's project directory.

### Lockfile

Cargo will generate `Cargo.lock` inside the cache directory when dependencies
exist. This is acceptable because the generated package is part of the cache.

If reproducible script dependencies are important, users can include exact
versions in the embedded manifest. Future support for embedded lock data is
possible but not required for the base design.

## Error Handling

Errors should identify `rustx` as the failing layer when the failure happens
before the script binary starts.

Examples:

```text
rustx: missing script path
rustx: script not found: path/to/script.rs
rustx: failed to read script: path/to/script.rs: ...
rustx: embedded cargo manifest is missing closing --- marker
rustx: failed to create cache directory: ...
rustx: failed to execute cargo build: ...
rustx: cached binary was not produced: ...
```

Cargo compiler errors should be allowed to print normally. `rustx` should not
try to parse or rewrite compiler diagnostics.

## Dependency Policy

`rustx` should not add third-party dependencies for the native runner.

Implementation areas that should stay dependency-free:

- command-line parsing
- embedded manifest extraction
- generated manifest assembly
- cache key hashing
- metadata file read/write
- process spawning

If a local `cargo-x` crate becomes part of the author's toolchain, it may be
used intentionally for shared functionality. That should be a separate design
decision, not an accidental dependency introduced to make the runner easier to
write.

## Compatibility Strategy

### With Existing rustx Usage

Current README usage should continue to work:

```rust
#!/usr/bin/env rustx

fn main() {
    println!("Hello!");
}
```

Users should no longer need to install `cargo-script`.

### With cargo-script Scripts

Full `cargo-script` compatibility is not a design goal. The native runner should
focus on a clean, maintainable script format.

Optional compatibility features can be considered after the native format is
stable:

- `// cargo-deps:` dependency comments
- expression scripts
- loop scripts
- test/bench script modes

Each compatibility feature should be justified independently.

### With Future Cargo Script Stabilization

The embedded `---cargo ... ---` format is chosen to align with Cargo's official
single-file package direction. If Cargo stabilizes script support, `rustx`
should be able to either:

- keep using its own stable implementation, or
- delegate to stable Cargo script support when available, while preserving cache
  and CLI behavior where useful.

## Security and Trust Model

Running a Rust script is equivalent to compiling and running arbitrary code.
`rustx` should not imply sandboxing.

Important rules:

- never execute generated files before a successful build
- keep generated files under the rustx cache root
- avoid following unexpected paths from embedded manifest data
- do not delete outside the cache root during cleanup
- print generated paths only for debugging, not as an invitation to edit cache
  files manually

## Testing Plan

### Unit Tests

Cover pure functions:

- shebang stripping
- embedded manifest extraction
- default manifest generation
- package field filling
- stable hash generation
- cache key construction
- metadata parsing and serialization
- CLI argument splitting

### Integration Tests

Use `RUSTX_CACHE_DIR` pointing to a temporary directory.

Required cases:

- no arguments prints usage and exits nonzero
- simple hello script runs
- executable shebang script runs
- script arguments are forwarded
- `argv[0]` is the original script path on Unix
- nonzero script exit code is propagated
- script panic exits nonzero
- embedded manifest with dependency builds
- invalid embedded manifest reports Cargo's error
- missing closing manifest marker reports a rustx parse error
- cache hit does not invoke Cargo
- `--force` invokes Cargo again
- script content change invalidates cache
- `--release` uses a separate cache/build output
- `--clear-cache` removes rustx cache entries only

To test cache hits without relying on timing, place a fake `cargo` earlier in
`PATH` after the first successful build and verify that a second run succeeds
without invoking it.

### Manual Tests

Verify these on macOS/Linux:

```text
chmod +x hello.rs
./hello.rs
./hello.rs arg1 arg2
rustx --release hello.rs
rustx --clear-cache
```

Windows support should be tested separately because shebang execution is not a
native Windows convention, though `rustx script.rs` should still work.

## Implementation Notes

Recommended module split:

```text
src/main.rs
src/cli.rs
src/script.rs
src/manifest.rs
src/cache.rs
src/build.rs
src/exec.rs
```

For a very small crate, this can still be implemented in fewer files initially,
but the conceptual boundaries should remain clear.

Core data types:

```text
ScriptInput
  original_path
  absolute_path
  raw_contents
  manifest_block
  rust_source

GeneratedPackage
  cache_dir
  manifest_path
  source_path
  package_name
  binary_path

BuildOptions
  profile
  target
  cargo_flags
  force
```

Process flow:

```text
parse_cli
load_script
extract_manifest_and_source
compute_cache_key
ensure_generated_package
build_if_needed
execute_binary
exit_with_status
```

## Open Decisions

- Exact minimum supported Rust version.
- Default edition: likely `2021`, possibly `2024` in the future.
- Whether generated package names should include the source filename.
- Whether cache key should include complete `rustc -vV` output or only host and
  release.
- Whether embedded `[package] name = ...` should be honored or replaced.
- Whether `cargo-deps` compatibility is worth supporting at all.
- Whether release builds should be requested by CLI only or via embedded
  manifest/profile configuration.
