use std::path::{Path, PathBuf};

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    tracing, ColliderCommand,
};
use collider_common::{
    miette::{self, Context, IntoDiagnostic, Result},
    smol::{self, fs, process::Command},
};
use collider_electron::{Electron, ElectronOpts};
use flate2::read::GzDecoder;
use tar::Archive;

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
        let proj_dir = build_dir.join("project");
        let copied_electron = electron.copy_files(&build_dir.join("release")).await?;
        self.stage_proj(&self.path, &proj_dir).await?;
        self.rebuild(&copied_electron).await?;
        Ok(copied_electron)
    }

    async fn rebuild(&self, electron: &Electron) -> Result<()> {
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
            .current_dir(&self.path)
            .status()
            .await
            .into_diagnostic()
            .context("Executing npx Command itself failed.")?;

        if status.success() {
            Ok(())
        } else {
            miette::bail!("node_modules rebuild failed.")
        }
    }

    async fn stage_proj(&self, from: &Path, to: &Path) -> Result<()> {
        tracing::info!("Staging current app before packaging");
        let npm_path = which::which("npm").into_diagnostic().context(
            "Failed to find npm command while packaging project. NPM/npx are required by collider.",
        )?;
        tracing::debug!("npm path: {}", npm_path.display());

        // First, we stage the current app into the staging folder by packing
        // it and re-extracting it. This gets rid of unneeded crud.
        tracing::debug!("Packing Electron project at {}", from.display());
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(&npm_path);
            cmd
        } else {
            Command::new(&npm_path)
        };
        let output = cmd
            .arg("pack")
            .current_dir(from)
            .output()
            .await
            .into_diagnostic()
            .context("Spawning npm pack Command itself failed.")?;

        if !output.status.success() {
            miette::bail!("packing app for staging failed.")
        }

        // Now that we packed, we need to extract the tarball into the staging directory and remove the tarball.
        let tarball_name = String::from_utf8(output.stdout)
            .into_diagnostic()
            .context("Failed to parse staging tarball filename as utf8. This is probably a bug in NPM itself.")?;
        let tarball = from.join(&tarball_name.trim());
        let to_clone = to.to_owned();
        tracing::debug!(
            "Extracting packed NPM project from {} to {}",
            tarball.display(),
            to.display()
        );
        smol::unblock(move || {
            // TODO: error handling
            let mut ar = Archive::new(GzDecoder::new(
                std::fs::File::open(&tarball).expect("BUG: Where did the tarball go?"),
            ));
            ar.unpack(&to_clone)
        })
        .await
        .into_diagnostic()
        .context("Failed to extract staging tarball.")?;

        let from_nm = from.join("node_modules");
        let to_nm = to.join("node_modules");
        tracing::debug!(
            "Copying node_modules folder from {} to {}",
            from_nm.display(),
            to_nm.display()
        );
        fs::create_dir_all(&to_nm)
            .await
            .into_diagnostic()
            .context("Failed to create destination directory for copied node_modules in staging")?;
        smol::unblock(move || {
            let mut opts = fs_extra::dir::CopyOptions::new();
            opts.overwrite = true;
            opts.content_only = true;
            fs_extra::dir::copy(from_nm, to_nm, &opts)
        })
        .await
        .into_diagnostic()
        .context("Failed to copy over node_modules")?;

        // Once we've moved node_modules, we do a prune and a rebuild.
        tracing::debug!(
            "Pruning node_modules directory at {}",
            to.join("node_modules").display()
        );
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(&npm_path);
            cmd
        } else {
            Command::new(&npm_path)
        };
        let status = cmd
            .arg("prune")
            .arg("--omit")
            .arg("dev")
            .current_dir(to)
            .status()
            .await
            .into_diagnostic()
            .context("Spawning npm Command itself failed.")?;

        if !status.success() {
            miette::bail!("pruning staging folder failed.")
        }

        Ok(())
    }
}
