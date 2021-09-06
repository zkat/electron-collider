use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    log, ColliderCommand,
};
use collider_common::miette::Result;

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct StartCmd {
    #[clap(long, short, about = "Electron version to use.")]
    electron_version: Option<String>,
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
        println!("Starting your application...");
        if let Some(version) = self.electron_version {
            println!("Using electron@{}", version);
            println!("Setting up Electron version...");
        }
        println!("...");
        println!(
            "Application started. Debug information will be printed here. Press Ctrl+C to exit."
        );
        Ok(())
    }
}
