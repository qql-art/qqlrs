use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::Context;
use hex_literal::hex;
use image::ImageFormat;
use raqote::DrawTarget;

// Set this environment variable to any non-empty string to write golden files (trivially passing
// the test) instead of checking them.
const ENV_UPDATE_GOLDENS: &str = "QQLRS_UPDATE_GOLDENS";

const GOLDEN_WIDTH: i32 = 800;

fn test_golden(seed: [u8; 32]) -> anyhow::Result<()> {
    let golden_filepath = PathBuf::from_iter([
        env!("CARGO_MANIFEST_DIR"),
        "goldens",
        format!("0x{}.png", hex::encode(seed)).as_str(),
    ]);

    let color_db = qql::color::ColorDb::from_bundle();
    let mut config = qql::config::Config::default();
    config.chunks = "2x2".parse().unwrap();

    let canvas = qql::art::draw(&seed, &color_db, &config, GOLDEN_WIDTH, |_| {}).canvas;
    if std::env::var_os(ENV_UPDATE_GOLDENS).is_some_and(|v| !v.is_empty()) {
        write_golden(&canvas, golden_filepath.as_ref())
    } else {
        check_golden(&canvas, golden_filepath.as_ref())
    }
}

fn write_golden(dt: &DrawTarget, golden_filepath: &Path) -> anyhow::Result<()> {
    dt.write_png(&golden_filepath)
        .context("Failed to write golden PNG")
}

fn check_golden(dt: &DrawTarget, golden_filepath: &Path) -> anyhow::Result<()> {
    let reader = BufReader::new(
        File::open(golden_filepath)
            .with_context(|| format!("Failed to read golden at {}", golden_filepath.display()))?,
    );
    let reader = image::io::Reader::with_format(reader, ImageFormat::Png);
    let golden = reader
        .decode()
        .context("Failed to decode image")?
        .into_rgba8();

    assert_eq!(
        (dt.width() as u32, dt.height() as u32),
        (golden.width(), golden.height())
    );

    let actual_pixels = dt.get_data().iter();
    let golden_pixels = golden.enumerate_pixels();
    for (actual_px, (x, y, golden_px)) in actual_pixels.zip(golden_pixels) {
        let [ab, ag, ar, _aa] = actual_px.to_le_bytes();
        let [gr, gg, gb, _ga] = golden_px.0;
        // Use a simple L-infinity norm for now. Can refine if we need to.
        assert_px_close((x, y), (ar, ag, ab), (gr, gg, gb));
    }

    Ok(())
}

fn assert_px_close((x, y): (u32, u32), actual: (u8, u8, u8), golden: (u8, u8, u8)) {
    const THRESHOLD: u32 = 16;
    let dr = channel_delta(actual.0, golden.0);
    let dg = channel_delta(actual.1, golden.1);
    let db = channel_delta(actual.2, golden.2);
    if dr > THRESHOLD || dg > THRESHOLD || db > THRESHOLD {
        panic!(
            "at ({}, {}): expected ~{:?}, got {:?}; max allowed deviation is {}",
            x, y, golden, actual, THRESHOLD
        );
    }
}

fn channel_delta(u: u8, v: u8) -> u32 {
    ((u as i32) - (v as i32)).abs() as u32
}

// Pick some QQLs to cover all the `Structure::*` and `ColorMode::*` options, as well as the linear
// and radial variants of `FlowField::*`, since those have the most kinds of distinct codepaths.
// Within those constraints, select for QQLs that render quickly and generate small output files.

/// QQL #77 uses `Structure::Formation`, `FlowField::RandomLinear`, and `ColorMode::Zebra`.
#[test]
fn golden_qql077() -> anyhow::Result<()> {
    let seed = hex!("b788f929c27e0a6e9abfc2a66ad878d73a930d128e1b0f08e009ffff10d10d4b");
    test_golden(seed)
}

/// QQL #219 uses `Structure::Shadows`, `FlowField::Circular`, and `ColorMode::Simple`.
#[test]
fn golden_qql219() -> anyhow::Result<()> {
    let seed = hex!("33c9371d25ce44a408f8a6473fbad86bf81e1a17a2e52c90cf66ffff1296712e");
    test_golden(seed)
}

/// QQL #234 uses `Structure::Orbital`, `FlowField::Circular`, and `ColorMode::Stacked`.
#[test]
fn golden_qql234() -> anyhow::Result<()> {
    let seed = hex!("4c61496e282ba45975b6863f14aeed35d686abfe78273b39ee44ffff146a6246");
    test_golden(seed)
}
