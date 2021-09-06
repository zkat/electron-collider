use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    log, ColliderCommand,
};
use collider_common::miette::Result;

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct PackCmd {
    #[clap(from_global)]
    loglevel: log::LevelFilter,
    #[clap(from_global)]
    quiet: bool,
    #[clap(from_global)]
    json: bool,
}

#[async_trait]
impl ColliderCommand for PackCmd {
    async fn execute(self) -> Result<()> {
        println!("Hello world!");
        Ok(())
    }
}
