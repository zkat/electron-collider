use std::path::PathBuf;

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    ColliderCommand,
};
use collider_common::{
    miette::{Context, IntoDiagnostic, Result},
    smol::fs,
};
use collider_electron::{Electron, ElectronOpts};

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct PackCmd {
    #[clap(
        about = "Path to Electron app. Must be an index.js file, a folder containing a package.json file, a folder containing an index.json file, and .html/.htm file, or an http/https/file URL.",
        default_value = "."
    )]
    path: String,

    #[clap(
        about = "Directory to write packaged output files to.",
        default_value = "collider-out",
        short,
        long
    )]
    output: PathBuf,

    #[clap(long, short, about = "Force download of the Electron binary.")]
    force: bool,

    #[clap(
        long,
        short = 'p',
        about = "Include prerelease versions when trying to find a version match."
    )]
    include_prerelease: bool,

    #[clap(long, short, about = "GitHub API Token (no permissions needed)")]
    github_token: Option<String>,

    #[clap(from_global)]
    quiet: bool,

    #[clap(from_global)]
    json: bool,
}

#[async_trait]
impl ColliderCommand for PackCmd {
    async fn execute(self) -> Result<()> {
        let out = self.output.clone();
        fs::create_dir_all(&out)
            .await
            .into_diagnostic()
            .context("Failed to create output directory")?;

        let electron = self.ensure_electron().await?;
        let electron_dir = electron
            .exe()
            .parent()
            .expect("BUG: This should definitely have a parent directory.")
            .to_owned();
        let dirname = electron_dir
            .file_name()
            .expect("BUG: This should have a file name.");
        let build_dir = out.join("electron-builds").join(dirname);
        let copied_electron = electron.copy_files(&build_dir).await?;

        Ok(())
    }
}

impl PackCmd {
    async fn ensure_electron(&self) -> Result<Electron> {
        let mut opts = ElectronOpts::new()
            .force(self.force)
            .include_prerelease(self.include_prerelease);
        if let Some(token) = &self.github_token {
            opts = opts.github_token(token.to_owned());
        }

        let electron = opts.ensure_electron().await?;
        Ok(electron)
    }
}
