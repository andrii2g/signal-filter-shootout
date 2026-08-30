//! Phase 0 command-line bootstrap.

use clap::Parser;

/// Compare scalar online filters on sensor data and PCM WAV audio.
///
/// Subcommands are added in their implementation phases so the advertised CLI
/// always reflects behavior that is actually available.
#[derive(Debug, Parser)]
#[command(name = "signal-filter-shootout", version, about)]
pub(crate) struct Cli;

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn help_contains_package_identity() {
        let mut command = Cli::command();
        let mut help = Vec::new();

        command.write_long_help(&mut help).expect("write help");
        let help = String::from_utf8(help).expect("help is UTF-8");

        assert!(help.contains("signal-filter-shootout"));
        assert!(help.contains("Compare scalar online filters"));
    }
}
