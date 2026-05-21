//! ascii-agents-core: headless logic for the ascii-agents TUI.

pub mod id;
pub mod source;
pub mod sprite;
pub mod state;

pub use id::AgentId;
pub use source::{Activity, AgentEvent, Source as SourceTrait};
pub use sprite::{Frame, Palette, Pixel, Rgb, RgbBuffer, Sprite};
pub use state::reducer::{Reducer, Source};
pub use state::{ActivityState, AgentSlot, SceneState};
