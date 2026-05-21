use anyhow::Result;

use crate::state::SceneState;

pub trait Renderer {
    fn render(&mut self, scene: &SceneState) -> Result<()>;
}

#[cfg(feature = "test-renderer")]
pub mod test_renderer;
