use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    log, ColliderCommand,
};
use collider_common::miette::{bail, IntoDiagnostic, Result};
use collider_node_semver::{Range, Version};

use async_compat::CompatExt;

pub use errors::StartError;

mod errors;

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct StartCmd {
    #[clap(long, short, about = "Electron version to use.", default_value = "*")]
    electron_version: Range,
    #[clap(long, short, about = "GitHub API Token (no permissions needed)")]
    github_token: Option<String>,
    #[clap(
        long,
        short,
        about = "Include prerelease versions when trying to find a version match."
    )]
    include_prerelease: bool,
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
        let version = self.get_electron_version(&self.electron_version).await?;
        println!("Starting your application using electron@{}", version);
        println!("...");
        println!(
            "Application started. Debug information will be printed here. Press Ctrl+C to exit."
        );
        Ok(())
    }
}

impl StartCmd {
    async fn get_electron_version(&self, range: &Range) -> Result<Version> {
        let mut crab = octocrab::OctocrabBuilder::new();
        if let Some(token) = &self.github_token {
            crab = crab.personal_token(token.clone());
        }
        let crab = crab.build().into_diagnostic()?;
        for page in 0u32.. {
            let tags = crab
                .repos("electron", "electron")
                .list_tags()
                .per_page(100)
                .page(page)
                .send()
                .compat()
                .await
                .map_err(map_octocrab_error)?
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
                        .map_err(map_octocrab_error)
                    {
                        Ok(_) => return Ok(version),
                        Err(err @ StartError::GitHubApiLimit(_)) => bail!(err),
                        Err(_) => {}
                    }
                }
            }
        }
        bail!("No Electron version found in range {}", range);
    }
}

fn map_octocrab_error(err: octocrab::Error) -> StartError {
    match err {
        octocrab::Error::GitHub {
            source: ref gh_err, ..
        } if gh_err.message.contains("rate limit exceeded") => {
            StartError::GitHubApiLimit(gh_err.clone())
        }
        _ => StartError::GitHubApiError(err),
    }
}
