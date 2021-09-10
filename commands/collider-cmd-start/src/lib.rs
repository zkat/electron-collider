use std::path::{Path, PathBuf};

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    tracing, ColliderCommand,
};
use collider_common::{
    directories::ProjectDirs,
    miette::Result,
    serde::Deserialize,
    serde_json,
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

        let exe = self.get_electron_exe(&range).await?;
        tracing::debug!("Launching executable at {}", exe.display());
        if !self.quiet && !self.json {
            println!(
                "Starting application. Debug information will be printed here. Press Ctrl+C to exit."
            );
        }
        self.exec_electron(&exe).await?;
        Ok(())
    }
}

impl StartCmd {
    async fn get_electron_exe(&self, range: &Range) -> Result<PathBuf, StartError> {
        let dirs = ProjectDirs::from("", "", "collider").ok_or(StartError::NoProjectDir)?;

        // First, we check to see if we can get a concrete version based on
        // what we have. This is a fast path that completely avoids external
        // requests.
        tracing::debug!("Looking up current collider version.");
        if let Some(version) = self.current_collider_version().await? {
            if !self.force && range.satisfies(&version) {
                let triple = self.get_target_triple(&version)?;
                let exe = dirs
                    .data_local_dir()
                    .join(&triple)
                    .join(self.get_exe_name());
                if fs::metadata(&exe).await.is_ok() {
                    return Ok(exe);
                }
            }
        }

        tracing::debug!("Current collider version missing or not useable. Looking up matching Electron releases on GitHub");
        let (version, release) = self.get_electron_release(range).await?;
        let triple = self.get_target_triple(&version)?;
        let dest = dirs.data_local_dir().join(&triple).to_owned();

        if !self.quiet && !self.json {
            println!("Selected electron@{} ({})", version, triple);
        } else if self.json {
            tracing::info!(
                "Selected electron@{version} ({triple})",
                version = version,
                triple = triple
            );
        }

        let zip = self.pick_electron_zip(&version, &release, &triple)?;
        self.ensure_electron(&dirs, &dest, &zip, &triple).await
    }

    async fn current_collider_version(&self) -> Result<Option<Version>, StartError> {
        for parent in std::env::current_exe()
            .map_err(StartError::CurrentExeFailure)?
            .parent()
            .expect("this should definitely have a parent")
            .ancestors()
        {
            let pkg_path = parent.join("package.json");
            if fs::metadata(&pkg_path).await.is_ok() {
                let pkg_src = fs::read_to_string(&pkg_path).await?;
                let pkg: PackageJson = serde_json::from_str(&pkg_src).map_err(|e| {
                    StartError::from_json_err(e, pkg_path.display().to_string(), pkg_src)
                })?;
                if pkg.name == "collider" {
                    return Ok(Some(pkg.version));
                }
            }
        }
        Ok(None)
    }

    async fn get_electron_release_from_tag(
        &self,
        crab: &octocrab::Octocrab,
        version: &Version,
        range: &Range,
    ) -> Result<Option<octocrab::models::repos::Release>, StartError> {
        if range.satisfies(version) && (!version.is_prerelease() || self.include_prerelease) {
            match crab
                .repos("electron", "electron")
                .releases()
                .get_by_tag(&format!("v{}", version))
                .compat()
                .await
                .map_err(StartError::from)
            {
                Ok(release) => return Ok(Some(release)),
                Err(err @ StartError::GitHubApiLimit(_)) => return Err(err),
                Err(_) => {}
            }
        }
        Ok(None)
    }

    async fn get_electron_release(
        &self,
        range: &Range,
    ) -> Result<(Version, octocrab::models::repos::Release), StartError> {
        let mut crab = octocrab::OctocrabBuilder::new();
        if let Some(token) = &self.github_token {
            crab = crab.personal_token(token.clone());
        }
        let crab = crab.build()?;

        if let Some(version) = self.current_collider_version().await? {
            if let Some(release) = self
                .get_electron_release_from_tag(&crab, &version, range)
                .await?
            {
                return Ok((version, release));
            }
        }

        // If we didn't find anything. It's time to query GitHub releases for the version we want.
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
                if let Some(release) = self
                    .get_electron_release_from_tag(&crab, &version, range)
                    .await?
                {
                    return Ok((version, release));
                }
            }
        }
        Err(StartError::MatchingVersionNotFound(range.clone()))
    }

    fn get_target_triple(&self, version: &Version) -> Result<String, StartError> {
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
        Ok(format!("v{}-{}-{}", version, platform, arch))
    }

    fn pick_electron_zip(
        &self,
        version: &Version,
        release: &octocrab::models::repos::Release,
        triple: &str,
    ) -> Result<Url, StartError> {
        let name = format!("electron-{}.zip", triple);
        release
            .assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.browser_download_url.clone())
            .ok_or_else(|| StartError::MissingElectronFiles {
                version: version.clone(),
                target: name,
            })
    }

    async fn ensure_electron(
        &self,
        dirs: &ProjectDirs,
        dest: &Path,
        zip: &Url,
        triple: &str,
    ) -> Result<PathBuf, StartError> {
        if self.force || fs::metadata(&dest).await.is_err() {
            let parent = dest.parent().expect("BUG: cache dir should have a parent");
            fs::create_dir_all(parent).await?;
            let cache = dirs.cache_dir();
            fs::create_dir_all(cache).await?;
            tracing::debug!("Fetching zip file from {}", zip);
            let mut res = reqwest::get(zip.to_string()).compat().await?;
            let zip_dest = cache.join(format!("electron-{}.zip", triple));
            tracing::debug!("Writing zip file to {}", zip_dest.display());
            let mut file = fs::File::create(&zip_dest).await?;
            let mut written = 0;
            while let Some(chunk) = res.chunk().compat().await? {
                file.write_all(chunk.as_ref()).await?;
                written += chunk.len();
            }
            file.flush().await?;
            std::mem::drop(file);
            tracing::debug!("Wrote {} bytes to zip file", written,);
            let dest = dest.to_owned();
            tracing::debug!("Extracting zip file to {}", dest.display());
            let zip_dest_clone = zip_dest.clone();
            smol::unblock(move || -> Result<(), StartError> {
                let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip_dest)?)?;
                // TODO: move this to its own method and do it manually, then
                // manually handle symlinks:
                // https://github.com/zip-rs/zip/pull/213
                archive.extract(&dest)?;
                Ok(())
            })
            .await?;
            tracing::debug!("Deleting zip file. We don't need it anymore.");
            fs::remove_file(&zip_dest_clone).await?;
        }
        Ok(dest.join(self.get_exe_name()))
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

#[derive(Debug, Deserialize)]
struct PackageJson {
    name: String,
    version: Version,
}
