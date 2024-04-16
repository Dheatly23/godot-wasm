mod classdb;
mod engine;
mod globalscope;

use anyhow::{bail, Result as AnyResult};

fn gate_unsafe(this: &crate::godot_component::GodotCtx) -> AnyResult<()> {
    if !this.allow_unsafe_behavior {
        bail!("Potentially unsafe operation attempted, aborting")
    }
    Ok(())
}
