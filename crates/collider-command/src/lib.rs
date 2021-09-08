use collider_common::miette::Result;

// Re-exports for common command deps:
pub use async_trait;
pub use clap;
pub use collider_config;
pub use owo_colors;
pub use tracing;

#[async_trait::async_trait]
pub trait ColliderCommand {
    async fn execute(self) -> Result<()>;
}
