use std::path::PathBuf;

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    log, ColliderCommand,
};
use collider_common::miette::{IntoDiagnostic, Result};

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct NewCmd {
    #[clap(about = "Path to create new Electron application in.")]
    path: PathBuf,
    #[clap(
        long,
        short = 't',
        default_value = "vanilla",
        about = "Template to use when scaffolding a new application."
    )]
    template: String,
    #[clap(from_global)]
    loglevel: log::LevelFilter,
    #[clap(from_global)]
    quiet: bool,
    #[clap(from_global)]
    json: bool,
}

#[async_trait]
impl ColliderCommand for NewCmd {
    async fn execute(self) -> Result<()> {
        let current_dir = std::env::current_dir().into_diagnostic()?;
        match self.template.as_ref() {
            "react" => println!(
                "Making a new React-based Electron app at {}",
                current_dir.join(self.path).display(),
            ),
            "vue" => println!(
                "Making a new Vue-based Electron app at {}",
                current_dir.join(self.path).display(),
            ),
            "vanilla" => println!(
                "Making a new VanillaJS-based Electron app at {}",
                current_dir.join(self.path).display(),
            ),
            template => panic!(
                "Unknown workload: {}, possible workloads are: react, vue, vanilla",
                template
            ),
        }
        Ok(())
    }
}
