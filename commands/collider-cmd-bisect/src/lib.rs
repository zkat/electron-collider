use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    tracing, ColliderCommand,
};
use collider_common::miette::Result;

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct BisectCmd {
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
        println!("Bisecting...");
        Ok(())
    }
}
