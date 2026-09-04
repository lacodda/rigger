//! rigger - one seat for all your projects and tasks.
//!
//! The command surface arrives one release at a time (see the roadmap in the
//! README); this is the entry point the release conveyor is proven on first.

use clap::Parser;

#[derive(Parser)]
#[command(name = "rigger", version, about, long_about = None)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
    println!(
        "rigger {}: no commands yet - the first release, v0.1.0, brings `init` and `project`.",
        env!("CARGO_PKG_VERSION")
    );
}
