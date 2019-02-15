# reventsource

[![Build Status](https://travis-ci.org/liuchong/rustx.svg?branch=master)](https://travis-ci.org/liuchong/rustx)

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
