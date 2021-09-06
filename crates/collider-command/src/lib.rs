use collider_common::miette::Result;

// Re-exports for common command deps:
pub use async_trait;
pub use clap;
pub use log;
pub use owo_colors;
pub use collider_config;

#[async_trait::async_trait]
pub trait ColliderCommand {
    async fn execute(self) -> Result<()>;
}
