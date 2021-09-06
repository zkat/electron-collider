use electron_collider::Collider;
use collider_common::{miette::Result, smol};

fn main() -> Result<()> {
    smol::block_on(Collider::load())?;
    Ok(())
}
