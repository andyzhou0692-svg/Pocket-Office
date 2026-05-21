//! ascii-agents-core: headless logic for the ascii-agents TUI.

pub mod id;
pub mod render;
pub mod source;
pub mod sprite;
pub mod state;

pub use id::AgentId;
pub use render::Renderer;
pub use source::{Activity, AgentEvent, Source, TaggedReceiver, TaggedSender};
pub use sprite::{Frame, Palette, Pixel, Rgb, RgbBuffer, Sprite};
pub use state::reducer::{Reducer, Transport};
pub use state::{ActivityState, AgentSlot, SceneState};
