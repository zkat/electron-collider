use std::path::Path;

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    tracing, ColliderCommand,
};
use collider_common::{
    miette::{Context, Result},
    serde::Deserialize,
    smol::process::Command,
};
use collider_electron::ElectronOpts;
use node_semver::{Range, Version};

pub use errors::StartError;

mod errors;

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct StartCmd {
    #[clap(
        about = "Path to Electron app. Must be an index.js file, a folder containing a package.json file, a folder containing an index.json file, and .html/.htm file, or an http/https/file URL.",
        default_value = "."
    )]
    path: String,

    #[clap(long, short, about = "Force download of the Electron binary.")]
    force: bool,

    #[clap(long, short, about = "Electron version to use.", default_value = "*")]
    using: String,

    #[clap(long, short, about = "GitHub API Token (no permissions needed)")]
    github_token: Option<String>,

    #[clap(long, short, about = "Open a REPL to the main process.")]
    interactive: bool,

    #[clap(long, short, about = "Print the Electron version being used.")]
    electron_version: bool,

    #[clap(long, short, about = "Print the Node ABI version.")]
    abi: bool,

    #[clap(
        long,
        short = 'p',
        about = "Include prerelease versions when trying to find a version match."
    )]
    include_prerelease: bool,

    #[clap(long, about = "Trace warnings")]
    trace_warnings: bool,

    #[clap(from_global)]
    quiet: bool,

    #[clap(from_global)]
    json: bool,
}

#[async_trait]
impl ColliderCommand for StartCmd {
    async fn execute(self) -> Result<()> {
        let range = self
            .using
            .parse::<Range>()
            .map_err(StartError::SemverError)?;

        let mut opts = ElectronOpts::new()
            .range(range)
            .force(self.force)
            .include_prerelease(self.include_prerelease);
        if let Some(token) = &self.github_token {
            opts = opts.github_token(token.to_owned());
        }

        let electron = opts.ensure_electron().await?;

        tracing::debug!("Launching executable at {}", electron.exe().display());
        if !self.quiet && !self.json {
            println!(
                "Starting application. Debug information will be printed here. Press Ctrl+C to exit."
            );
        }
        self.exec_electron(electron.exe()).await.with_context(|| {
            format!(
                "Failed to execute Electron binary at {}",
                electron.exe().display()
            )
        })?;
        Ok(())
    }
}

impl StartCmd {
    async fn exec_electron(&self, exe: &Path) -> Result<(), StartError> {
        let mut cmd = Command::new(exe);
        if self.abi {
            cmd.arg("--abi");
        } else if self.electron_version {
            cmd.arg("--version");
        } else {
            if self.trace_warnings {
                cmd.arg("--trace-warnings");
            }
            if self.interactive {
                cmd.arg("--interactive");
            }
            cmd.arg(&self.path);
        }
        let status = cmd.status().await?;
        if status.success() {
            Ok(())
        } else {
            Err(StartError::ElectronFailed)
        }
    }
}
