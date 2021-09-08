use std::path::{Path, PathBuf};

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    log, ColliderCommand,
};
use collider_common::{
    directories::ProjectDirs,
    miette::Result,
    smol::{self, fs, io::AsyncWriteExt, process::Command},
    surf::Url,
};
use node_semver::{Range, Version};

use async_compat::CompatExt;

pub use errors::StartError;

mod errors;

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct StartCmd {
    #[clap(
        about = "Path to Electron app. Must be an index.js file, a folder containing a package.json file, a folder containing an index.json file, and .html/.htm file, or an http/https/file URL.",
        default_value = "."
    )]
    path: PathBuf,

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
    loglevel: log::LevelFilter,

    #[clap(from_global)]
    quiet: bool,

    #[clap(from_global)]
    json: bool,
}

#[async_trait]
impl ColliderCommand for StartCmd {
    async fn execute(self) -> Result<()> {
        let (version, release) = self
            .get_electron_release(&self.using.parse().map_err(StartError::SemverError)?)
            .await?;
        log::info!("Selected electron@{}", version);
        let triple = self.get_target_triple(&release)?;
        let zip = self.pick_electron_zip(&version, &release, &triple)?;
        let dirs = ProjectDirs::from("", "", "collider").ok_or(StartError::NoProjectDir)?;
        let dest = dirs.data_local_dir().join(&triple).to_owned();
        self.ensure_electron(&dirs, &dest, &zip, &triple).await?;
        let exe = dest.join(self.get_exe_name());
        log::info!("Launching executable at {}", exe.display());
        println!(
            "Starting application. Debug information will be printed here. Press Ctrl+C to exit."
        );
        self.exec_electron(&exe).await?;
        Ok(())
    }
}

impl StartCmd {
    async fn get_electron_release(
        &self,
        range: &Range,
    ) -> Result<(Version, octocrab::models::repos::Release), StartError> {
        let mut crab = octocrab::OctocrabBuilder::new();
        if let Some(token) = &self.github_token {
            crab = crab.personal_token(token.clone());
        }
        let crab = crab.build()?;
        for page in 0u32.. {
            let tags = crab
                .repos("electron", "electron")
                .list_tags()
                .per_page(100)
                .page(page)
                .send()
                .compat()
                .await?
                .items;
            if tags.is_empty() {
                break;
            }
            for tag in tags.into_iter() {
                let version = tag.name[1..].parse::<Version>()?;
                if range.satisfies(&version)
                    && (!version.is_prerelease() || self.include_prerelease)
                {
                    match crab
                        .repos("electron", "electron")
                        .releases()
                        .get_by_tag(&tag.name)
                        .compat()
                        .await
                        .map_err(StartError::from)
                    {
                        Ok(release) => return Ok((version, release)),
                        Err(err @ StartError::GitHubApiLimit(_)) => return Err(err),
                        Err(_) => {}
                    }
                }
            }
        }
        Err(StartError::MatchingVersionNotFound(range.clone()))
    }

    fn get_target_triple(
        &self,
        release: &octocrab::models::repos::Release,
    ) -> Result<String, StartError> {
        let platform = match std::env::consts::OS {
            "windows" => "win32",
            "macos" => "darwin",
            "linux" => "linux",
            // TODO: wtf is "mas"?
            _ => return Err(StartError::UnsupportedPlatform(std::env::consts::OS.into())),
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "ia32",
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => return Err(StartError::UnsupportedArch(std::env::consts::ARCH.into())),
        };
        Ok(format!("{}-{}-{}", release.tag_name, platform, arch))
    }

    fn pick_electron_zip(
        &self,
        version: &Version,
        release: &octocrab::models::repos::Release,
        triple: &str,
    ) -> Result<Url> {
        let name = format!("electron-{}.zip", triple);
        Ok(release
            .assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.browser_download_url.clone())
            .ok_or_else(|| StartError::MissingElectronFiles {
                version: version.clone(),
                target: name,
            })?)
    }

    async fn ensure_electron(
        &self,
        dirs: &ProjectDirs,
        dest: &Path,
        zip: &Url,
        triple: &str,
    ) -> Result<(), StartError> {
        if self.force || fs::metadata(&dest).await.is_err() {
            let parent = dest.parent().expect("BUG: cache dir should have a parent");
            fs::create_dir_all(parent).await?;
            let cache = dirs.cache_dir();
            fs::create_dir_all(cache).await?;
            log::info!("Fetching zip file from {}", zip);
            let mut res = reqwest::get(zip.to_string()).compat().await?;
            let zip_dest = cache.join(format!("electron-{}.zip", triple));
            log::info!("Writing zip file to {}", zip_dest.display());
            let mut file = fs::File::create(&zip_dest).await?;
            // TODO: For some reason, this keeps failing like half the time
            // due to a broken zip file? I don't it at all. I think the best
            // thing to do is just to retry `ensure_electron` several times
            // unless it just keeps failing? See https://crates.io/crates/backoff
            while let Some(chunk) = res.chunk().compat().await? {
                file.write_all(&chunk[..]).await?;
            }
            std::mem::drop(file);
            log::info!("Zip file written.");
            let dest = dest.to_owned();
            log::info!("Extracting zip file to {}", dest.display());
            let zip_dest_clone = zip_dest.clone();
            smol::unblock(move || -> Result<(), StartError> {
                let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip_dest)?)?;
                archive.extract(&dest)?;
                Ok(())
            })
            .await?;
            log::info!("Deleting zip file. We don't need it anymore.");
            fs::remove_file(&zip_dest_clone).await?;
        }
        Ok(())
    }

    fn get_exe_name(&self) -> String {
        match std::env::consts::OS {
            "windows" => "electron.exe".into(),
            "macos" => "Electron.app/Contents/MacOS/Electron".into(),
            "linux" => "electron".into(),
            _ => "electron".into(),
        }
    }

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
