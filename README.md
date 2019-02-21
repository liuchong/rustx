# rustx

[![Build Status](https://travis-ci.org/liuchong/rustx.svg?branch=master)](https://travis-ci.org/liuchong/rustx)
[![APACHE licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)](./LICENSE-APACHE)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE-MIT)
[![crates.io](https://meritbadge.herokuapp.com/rustx)](https://crates.io/crates/rustx)
[![Released API docs](https://docs.rs/rustx/badge.svg)](https://docs.rs/rustx)

Rustx invoke cargo-script.

<https://doc.rust-lang.org/reference/crates-and-source-files.html>

## Usage

1. `cargo install rustx`, `cargo install cargo-script`

2. create a file, for example, `hello.rs` as below, then `chmod +x hello.rs`, `./hello.rs`.

``` rust
#!/usr/bin/env rustx

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
