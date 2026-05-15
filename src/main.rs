mod build;
mod cache;
mod cli;
mod manifest;
mod process;
mod runner;
mod script;

fn main() {
    std::process::exit(runner::run(std::env::args()));
}
