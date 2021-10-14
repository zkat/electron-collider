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
        about = "Path to the root of an Electron app. Must be a directory containing a package.json and any files you want to bundle into the app.",
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

    #[clap(
        about = "Path to a prebuilt ASAR file. By default, Collider will build it for you.",
        short,
        long
    )]
    asar: Option<PathBuf>,

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
        // Make sure we've downloaded & cached an electron version
        let electron = self.ensure_electron().await?;
        let asar = self.ensure_asar(&electron).await?;
        fs::create_dir_all(&out)
            .await
            .into_diagnostic()
            .context("Failed to create output directory")?;
        let rel_electron = self.prepare_release(&electron, &asar, &out).await?;
        println!("{:#?}", rel_electron);
        Ok(())
    }
}

impl PackCmd {
    async fn ensure_asar(&self, electron: &Electron) -> Result<PathBuf> {
        if let Some(asar) = &self.asar {
            return Ok(asar.clone());
        }
        self.prune_proj(&self.path).await?;
        self.rebuild_proj(&self.path, electron).await?;
        let asar_dest = self.path.join("node_modules").join("app.asar");
        self.pack_asar(&self.path, &asar_dest).await?;
        Ok(asar_dest)
    }

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

    async fn prepare_release(
        &self,
        electron: &Electron,
        asar: &Path,
        out: &Path,
    ) -> Result<Electron> {
        let electron_dir = electron
            .exe()
            .parent()
            .expect("BUG: This should definitely have a parent directory.")
            .to_owned();
        let dirname = electron_dir
            .file_name()
            .expect("BUG: This should have a file name.");
        let build_dir = out.join(dirname);
        let copied_electron = electron.copy_files(&build_dir.join("release")).await?;
        self.remove_default_app_asar(&copied_electron).await?;
        let asar_dest = build_dir.join("release").join("resources").join("app.asar");
        let opts = fs_extra::file::CopyOptions::new();
        fs_extra::file::copy(asar, &asar_dest, &opts).into_diagnostic()?;
        Ok(copied_electron)
    }

    async fn remove_default_app_asar(&self, electron: &Electron) -> Result<()> {
        let default_app = electron
            .exe()
            .parent()
            .expect("BUG: This should have a parent directory.")
            .join("resources")
            .join("default_app.asar");
        fs::remove_file(&default_app).await.into_diagnostic()?;
        Ok(())
    }

    async fn prune_proj(&self, proj_dir: &Path) -> Result<()> {
        tracing::info!("Pruning current node_modules down to only production dependencies.");
        // TODO: Instead of doing this, get a direct path to the npm-cli.js
        // file. This will help bypass the Terminate Batch Job b.s. on
        // Windows.
        let npm_path = which::which("npm").into_diagnostic().context(
            "Failed to find npm command while packaging project. NPM/npx are required by collider.",
        )?;

        // TODO: pnpm and Yarn support. See https://github.com/zkochan/which-pm. For now, just use NPM :)
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(npm_path);
            cmd
        } else {
            Command::new(npm_path)
        };

        let status = cmd
            .arg("install")
            .arg("--production")
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

    async fn pack_asar(&self, proj_dir: &Path, dest: &Path) -> Result<()> {
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
            .arg("asar")
            .arg("pack")
            .arg(proj_dir)
            .arg(dest)
            .current_dir(proj_dir)
            .status()
            .await
            .into_diagnostic()
            .context("Failed to spawn npx itself.")?;

        if !status.success() {
            miette::bail!("Packaging up .asar failed.")
        }

        Ok(())
    }
}
