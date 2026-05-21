//! Embeds the bundled top-down sprite pack into the binary at compile time.

use anyhow::Result;
use ascii_agents_core::sprite::format::{load_pack_from_strings, Pack};

pub fn load_default_pack() -> Result<Pack> {
    let pack_toml = include_str!("../../../../assets/sprites/default/pack.toml");
    let seated     = include_str!("../../../../assets/sprites/default/seated.sprite");
    let typing_0   = include_str!("../../../../assets/sprites/default/typing_0.sprite");
    let typing_1   = include_str!("../../../../assets/sprites/default/typing_1.sprite");
    let standing   = include_str!("../../../../assets/sprites/default/standing.sprite");
    let walking_0  = include_str!("../../../../assets/sprites/default/walking_0.sprite");
    let walking_1  = include_str!("../../../../assets/sprites/default/walking_1.sprite");
    let desk       = include_str!("../../../../assets/sprites/default/desk.sprite");
    let plant      = include_str!("../../../../assets/sprites/default/plant.sprite");
    let couch      = include_str!("../../../../assets/sprites/default/couch.sprite");
    let coffee     = include_str!("../../../../assets/sprites/default/coffee.sprite");

    load_pack_from_strings(
        pack_toml,
        &[
            ("seated.sprite", seated),
            ("typing_0.sprite", typing_0),
            ("typing_1.sprite", typing_1),
            ("standing.sprite", standing),
            ("walking_0.sprite", walking_0),
            ("walking_1.sprite", walking_1),
            ("desk.sprite", desk),
            ("plant.sprite", plant),
            ("couch.sprite", couch),
            ("coffee.sprite", coffee),
        ],
    )
}
