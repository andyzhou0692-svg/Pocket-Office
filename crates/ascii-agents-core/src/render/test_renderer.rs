use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::render::Renderer;
use crate::state::SceneState;

/// Captures every SceneState handed to it. Used in e2e tests.
#[derive(Clone, Default)]
pub struct TestRenderer {
    pub snapshots: Arc<Mutex<Vec<SceneState>>>,
}

impl TestRenderer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.snapshots.lock().unwrap().len()
    }
}

impl Renderer for TestRenderer {
    fn render(&mut self, scene: &SceneState) -> Result<()> {
        self.snapshots.lock().unwrap().push(scene.clone());
        Ok(())
    }
}
