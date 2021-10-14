use std::path::{Path, PathBuf};

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    tracing, ColliderCommand,
};
use collider_common::{
    miette::{self, Context, IntoDiagnostic, Result},
    smol::{fs, process::Command},
};
use collider_electron::{Electron, ElectronOpts};

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct PackCmd {
    #[clap(
        about = "Path to Electron app. Must be an index.js file, a folder containing a package.json file, or a folder containing an index.json file, and .html/.htm file. URLs are not supported by this tool.",
        default_value = "."
    )]
    path: PathBuf,

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

        let electron = self.ensure_electron(&out).await?;
        println!("{:#?}", electron);
        Ok(())
    }
}

impl PackCmd {
    async fn ensure_electron(&self, out: &Path) -> Result<Electron> {
        let mut opts = ElectronOpts::new()
            .force(self.force)
            .include_prerelease(self.include_prerelease);
        if let Some(token) = &self.github_token {
            opts = opts.github_token(token.to_owned());
        }

        let electron = opts.ensure_electron().await?;
        let electron_dir = electron
            .exe()
            .parent()
            .expect("BUG: This should definitely have a parent directory.")
            .to_owned();
        let dirname = electron_dir
            .file_name()
            .expect("BUG: This should have a file name.");
        let build_dir = out.join("electron-builds").join(dirname);
        let copied_electron = electron.copy_files(&build_dir.join("release")).await?;
        self.prune_proj(&self.path).await?;
        self.rebuild_proj(&self.path, &copied_electron).await?;
        Ok(copied_electron)
    }

    async fn prune_proj(&self, proj_dir: &Path) -> Result<()> {
        tracing::info!("Pruning current node_modules down to only production dependencies.");
        let npm_path = which::which("npm").into_diagnostic().context(
            "Failed to find npm command while packaging project. NPM/npx are required by collider.",
        )?;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(npm_path);
            cmd
        } else {
            Command::new(npm_path)
        };

        let status = cmd
            .arg("prune")
            .arg("--omit")
            .arg("dev")
            .arg("--quiet")
            .current_dir(proj_dir)
            .status()
            .await
            .into_diagnostic()
            .context("Failed to spawn NPM itself.")?;

        if !status.success() {
            miette::bail!("node_modules pruning failed.")
        }

        Ok(())
    }

    async fn rebuild_proj(&self, proj_dir: &Path, electron: &Electron) -> Result<()> {
        tracing::info!("Rebuilding node_modules for target platform.");
        let npx_path = which::which("npx").into_diagnostic().context(
            "Failed to find npx command while packaging project. NPM/npx are required by collider.",
        )?;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(npx_path);
            cmd
        } else {
            Command::new(npx_path)
        };

        let status = cmd
            .arg("electron-rebuild")
            .arg("--arch")
            .arg(electron.arch())
            .arg("--platform")
            .arg(electron.os())
            .current_dir(proj_dir)
            .status()
            .await
            .into_diagnostic()
            .context("Failed to spawn npx itself.")?;

        if !status.success() {
            miette::bail!("node_modules rebuild failed.")
        }

        Ok(())
    }
}
