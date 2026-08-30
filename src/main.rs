//! CLI entry point and, in later phases, command orchestration.

#![forbid(unsafe_code)]

mod cli;

use clap::Parser;

use crate::cli::Cli;

fn main() {
    let _cli = Cli::parse();
}
