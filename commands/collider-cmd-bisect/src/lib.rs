use std::path::PathBuf;

use async_compat::CompatExt;

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    tracing, ColliderCommand,
};

use collider_common::{
    miette::{IntoDiagnostic, Result},
    serde::Deserialize,
    smol::process::Command,
};

use collider_electron::ElectronOpts;

use dialoguer::{theme::ColorfulTheme, Confirm};

use node_semver::{Range, Version};

pub use errors::BisectError;

mod errors;

#[derive(Deserialize)]
struct ElectronVersion {
    version: Version,
}

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct BisectCmd {
    #[clap(
        about = "Path to Electron app that causes the issue. Must be an index.js file, a folder containing a package.json file, a folder containing an index.json file, and .html/.htm file, or an http/https/file URL.",
        default_value = "."
    )]
    path: PathBuf,

    #[clap(
        long,
        short,
        about = "Electron version to start bisecting at (Last \"known good\" version).",
        default_value = "*"
    )]
    start: String,

    #[clap(
        long,
        short,
        about = "Electron version to end bisecting at (First \"known bad\" version).",
        default_value = "*"
    )]
    end: String,

    #[clap(
        long,
        short,
        about = "Run bisect in interactive mode.  Otherwise, the Electron app will need to return a non-zero exit code to indicate failure."
    )]
    interactive: bool,

    #[clap(from_global)]
    verbosity: tracing::Level,
    #[clap(from_global)]
    quiet: bool,
    #[clap(from_global)]
    json: bool,
}

#[async_trait]
impl ColliderCommand for BisectCmd {
    async fn execute(self) -> Result<()> {
        let versions_response = reqwest::get("https://releases.electronjs.org/releases.json")
            .compat()
            .await
            .into_diagnostic()?;
        let all_versions: Vec<ElectronVersion> =
            versions_response.json().await.into_diagnostic()?;
        let start_version = self.get_version(
            &self.start,
            &all_versions[all_versions.len() - 1].version.to_string(),
        )?;
        let end_version = self.get_version(&self.end, &all_versions[0].version.to_string())?;
        let mut bisect_versions: Vec<ElectronVersion> = all_versions
            .into_iter()
            .filter(|version| {
                !version.version.is_prerelease()
                    && version.version >= start_version
                    && version.version <= end_version
            })
            .collect();
        bisect_versions.reverse();

        println!("Bisecting... {} to {}", start_version, end_version);

        let mut min_rev = 0;
        let mut max_rev = bisect_versions.len() - 1;
        let mut pivot = (max_rev - min_rev) / 2;
        let mut is_bisect_over = false;
        while !is_bisect_over {
            if max_rev - min_rev <= 1 {
                is_bisect_over = true;
            }
            let target_version = &bisect_versions[pivot];
            println!("Testing {}", target_version.version);
            let range = target_version
                .version
                .to_string()
                .parse::<Range>()
                .map_err(BisectError::SemverError)?;
            let opts = ElectronOpts::new().range(range).include_prerelease(true);

            let electron = opts.ensure_electron().await?;
            println!(
                "Successfully got {}; now running test",
                target_version.version
            );
            let mut cmd = Command::new(electron.exe());
            cmd.arg(&self.path);
            let status = cmd.status().await.into_diagnostic()?;
            let mut test_passed = status.success();

            if self.interactive {
                test_passed = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(format!(
                        "Did test case pass for {}?",
                        target_version.version
                    ))
                    .interact()
                    .into_diagnostic()?;
            }

            if test_passed {
                println!("{} passed testing.", target_version.version);
                let up_pivot = ((max_rev - pivot) / 2) + pivot;
                min_rev = pivot;
                if up_pivot != max_rev && up_pivot != pivot {
                    pivot = up_pivot;
                } else {
                    is_bisect_over = true;
                }
            } else {
                println!("{} failed testing.", target_version.version);
                let down_pivot = ((pivot - min_rev) / 2) + min_rev;
                max_rev = pivot;
                if down_pivot != min_rev && down_pivot != pivot {
                    pivot = down_pivot;
                } else {
                    is_bisect_over = true;
                }
            }
        }
        println!("Bisect complete. Check the range {min_rev}...{max_rev} at https://github.com/electron/electron/compare/v{min_rev}...v{max_rev}", min_rev = &bisect_versions[min_rev].version, max_rev = &bisect_versions[max_rev].version);
        Ok(())
    }
}

impl BisectCmd {
    fn get_version(
        &self,
        specified_version: &str,
        default_version: &str,
    ) -> Result<Version, BisectError> {
        if specified_version == "*" {
            Ok(default_version.parse()?)
        } else {
            Ok(specified_version.parse()?)
        }
    }
}
