use std::path::{Path, PathBuf};

use async_compat::CompatExt;
use collider_common::{
    directories::ProjectDirs,
    serde::Deserialize,
    serde_json,
    smol::{self, fs, io::AsyncWriteExt},
    tracing,
};
use node_semver::{Range, Version};

use errors::ElectronError;
use reqwest::Url;

mod errors;

#[derive(Debug, Clone, Deserialize)]
struct PackageJson {
    name: String,
    version: Version,
}

pub struct Electron {
    exe: PathBuf,
    version: Version,
    os: String,
    arch: String,
}

impl Electron {
    pub fn exe(&self) -> &Path {
        &self.exe
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn os(&self) -> &str {
        &self.os
    }

    pub fn arch(&self) -> &str {
        &self.arch
    }
}

pub struct ElectronOpts {
    force: Option<bool>,
    range: Option<Range>,
    include_prerelease: Option<bool>,
    github_token: Option<String>,
}

impl Default for ElectronOpts {
    fn default() -> Self {
        Self {
            force: None,
            range: None,
            include_prerelease: None,
            github_token: None,
        }
    }
}

impl ElectronOpts {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn force(mut self, force: bool) -> Self {
        self.force = Some(force);
        self
    }

    pub fn range(mut self, range: Range) -> Self {
        self.range = Some(range);
        self
    }

    pub fn include_prerelease(mut self, include_prerelease: bool) -> Self {
        self.include_prerelease = Some(include_prerelease);
        self
    }

    pub fn github_token(mut self, github_token: String) -> Self {
        self.github_token = Some(github_token);
        self
    }

    pub async fn build(self) -> Result<Electron, ElectronError> {
        let dirs = ProjectDirs::from("", "", "collider").ok_or(ElectronError::NoProjectDir)?;
        let range = self.range.clone().unwrap_or_else(Range::any);
        let os = match std::env::consts::OS {
            "windows" => "win32",
            "macos" => "darwin",
            "linux" => "linux",
            // TODO: "mas"?
            _ => {
                return Err(ElectronError::UnsupportedPlatform(
                    std::env::consts::OS.into(),
                ))
            }
        }
        .to_string();
        let arch = match std::env::consts::ARCH {
            "x86" => "ia32",
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => {
                return Err(ElectronError::UnsupportedArch(
                    std::env::consts::ARCH.into(),
                ))
            }
        }
        .to_string();

        // First, we check to see if we can get a concrete version based on
        // what we have. This is a fast path that completely avoids external
        // requests.
        tracing::debug!("Looking up current collider version.");
        if let Some(version) = self.current_collider_version().await? {
            if !self.force.unwrap_or(false) && range.satisfies(&version) {
                let triple = self.get_target_triple(&version, &os, &arch)?;
                let exe = dirs
                    .data_local_dir()
                    .join(&triple)
                    .join(self.get_exe_name());
                if fs::metadata(&exe).await.is_ok() {
                    return Ok(Electron {
                        exe,
                        os,
                        arch,
                        version: version.clone(),
                    });
                }
            }
        }

        tracing::debug!("Current collider version missing or not useable. Looking up matching Electron releases on GitHub");
        let (version, release) = self.get_electron_release(&range).await?;
        let triple = self.get_target_triple(&version, &os, &arch)?;
        let dest = dirs.data_local_dir().join(&triple).to_owned();

        tracing::info!(
            "Selected electron@{version} ({triple})",
            version = version,
            triple = triple
        );

        let zip = self.pick_electron_zip(&version, &release, &triple)?;
        let exe = self.ensure_electron(&dirs, &dest, &zip, &triple).await?;
        Ok(Electron {
            exe,
            version,
            os,
            arch,
        })
    }

    async fn current_collider_version(&self) -> Result<Option<Version>, ElectronError> {
        for parent in std::env::current_exe()
            .map_err(ElectronError::CurrentExeFailure)?
            .parent()
            .expect("this should definitely have a parent")
            .ancestors()
        {
            let pkg_path = parent.join("package.json");
            if fs::metadata(&pkg_path).await.is_ok() {
                let pkg_src = fs::read_to_string(&pkg_path).await?;
                let pkg: PackageJson = serde_json::from_str(&pkg_src).map_err(|e| {
                    ElectronError::from_json_err(e, pkg_path.display().to_string(), pkg_src)
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
    ) -> Result<Option<octocrab::models::repos::Release>, ElectronError> {
        if range.satisfies(version)
            && (!version.is_prerelease() || self.include_prerelease.unwrap_or(false))
        {
            match crab
                .repos("electron", "electron")
                .releases()
                .get_by_tag(&format!("v{}", version))
                .compat()
                .await
                .map_err(ElectronError::from)
            {
                Ok(release) => return Ok(Some(release)),
                Err(err @ ElectronError::GitHubApiLimit(_)) => return Err(err),
                Err(_) => {}
            }
        }
        Ok(None)
    }

    async fn get_electron_release(
        &self,
        range: &Range,
    ) -> Result<(Version, octocrab::models::repos::Release), ElectronError> {
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
        Err(ElectronError::MatchingVersionNotFound(range.clone()))
    }

    fn get_target_triple(
        &self,
        version: &Version,
        os: &str,
        arch: &str,
    ) -> Result<String, ElectronError> {
        Ok(format!("v{}-{}-{}", version, os, arch))
    }

    fn pick_electron_zip(
        &self,
        version: &Version,
        release: &octocrab::models::repos::Release,
        triple: &str,
    ) -> Result<Url, ElectronError> {
        let name = format!("electron-{}.zip", triple);
        release
            .assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.browser_download_url.clone())
            .ok_or_else(|| ElectronError::MissingElectronFiles {
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
    ) -> Result<PathBuf, ElectronError> {
        if self.force.unwrap_or(false) || fs::metadata(&dest).await.is_err() {
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
            smol::unblock(move || -> Result<(), ElectronError> {
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
}
