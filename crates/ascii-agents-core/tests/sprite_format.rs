use std::path::Path;

use ascii_agents_core::sprite::format::{load_pack, parse_sprite_file};
use ascii_agents_core::sprite::{Palette, Rgb};

fn palette() -> Palette {
    let mut p = Palette::new();
    p.insert('A', Some(Rgb(1, 2, 3)));
    p.insert('B', Some(Rgb(4, 5, 6)));
    p.insert('.', None);
    p
}

#[test]
fn parses_two_frame_mini_sprite() {
    let src = std::fs::read_to_string("tests/fixtures/sprites/mini.sprite").unwrap();
    let frames = parse_sprite_file(&src, &palette()).unwrap();

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].width, 4);
    assert_eq!(frames[0].height, 2);
    assert_eq!(frames[0].pixels[0], Some(Rgb(1, 2, 3)));
    assert_eq!(frames[0].pixels[1], None);
    assert_eq!(frames[0].pixels[2], Some(Rgb(4, 5, 6)));
    assert_eq!(frames[0].pixels[3], None);
}

#[test]
fn rejects_unknown_palette_key() {
    let palette = palette();
    let src = "@frame 0\nA . ? .";
    let err = parse_sprite_file(src, &palette).unwrap_err();
    assert!(err.to_string().contains("unknown palette key"), "got: {err}");
}

#[test]
fn rejects_inconsistent_row_widths() {
    let palette = palette();
    let src = "@frame 0\nA . B .\nA . B";
    let err = parse_sprite_file(src, &palette).unwrap_err();
    assert!(err.to_string().contains("row width"), "got: {err}");
}

#[test]
fn loads_mini_pack() {
    let pack = load_pack(Path::new("tests/fixtures/sprites/mini_pack")).unwrap();
    let idle = pack.animation("idle").expect("idle animation");
    assert_eq!(idle.frame_ms, 500);
    assert_eq!(idle.frames.len(), 1);
    assert_eq!(idle.frames[0].width, 4);
}

#[test]
fn missing_animation_returns_none() {
    let pack = load_pack(Path::new("tests/fixtures/sprites/mini_pack")).unwrap();
    assert!(pack.animation("nope").is_none());
}

#[test]
fn default_pack_loads_with_three_animations() {
    let pack = load_pack(Path::new("../../assets/sprites/default")).unwrap();
    assert!(pack.animation("idle").is_some());
    assert!(pack.animation("typing").is_some());
    assert!(pack.animation("waiting").is_some());
    let typing = pack.animation("typing").unwrap();
    assert_eq!(typing.frames.len(), 3);
    assert_eq!(typing.frame_ms, 120);
    assert_eq!(typing.frames[0].width, 12);
    assert_eq!(typing.frames[0].height, 16);
}
