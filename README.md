# rustx

[![Build Status](https://github.com/liuchong/rustx/actions/workflows/rust.yml/badge.svg)](https://github.com/liuchong/rustx/actions/workflows/rust.yml)
[![APACHE licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)](./LICENSE-APACHE)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE-MIT)
[![crates.io](https://img.shields.io/crates/v/rustx.svg)](https://crates.io/crates/rustx)
[![Released API docs](https://docs.rs/rustx/badge.svg)](https://docs.rs/rustx)

Rustx runs self-contained Rust scripts.

<https://doc.rust-lang.org/reference/input-format.html>

## Usage

1. `cargo install rustx`

2. create a file, for example, `hello.rs` as below, then `chmod +x hello.rs`, `./hello.rs`.

``` rust
#!/usr/bin/env rustx

fn main() {
    println!("Hello!");
}
```

Scripts can embed Cargo manifest data in the same file:

``` rust
#!/usr/bin/env rustx
---cargo
[package]
edition = "2021"

[dependencies]
---

fn main() {
    println!("Hello!");
}
```

## License

Licensed under either of these:

 * Apache License Version 2.0 [LICENSE-APACHE](LICENSE-APACHE)
 * MIT License [LICENSE-MIT](LICENSE-MIT)

### Contributing

Please sign a cla, thanks!
