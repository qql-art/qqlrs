use core::fmt::Debug;
use std::{fmt::Display, str::FromStr};

use clap::Parser;

#[derive(Parser)]
struct Opts {
    seed: Seed,
    #[clap(short, default_value = "2400")]
    width: i32,
    #[clap(flatten)]
    config: qql::config::Config,
}

#[derive(Copy, Clone)]
struct Seed(pub [u8; 32]);
impl Seed {
    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl FromStr for Seed {
    type Err = anyhow::Error;
    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("0x") {
            s = &s[2..];
        }
        let bytes: Vec<u8> = hex::decode(s)?;
        let bytes: [u8; 32] = <[u8; 32]>::try_from(bytes).unwrap();
        Ok(Seed(bytes))
    }
}
impl Debug for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("0x")?;
        let mut buf = [0u8; 2 * 32];
        hex::encode_to_slice(self.0, &mut buf).unwrap();
        f.write_str(std::str::from_utf8(&buf).unwrap())
    }
}
impl Display for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Debug>::fmt(self, f)
    }
}

fn main() {
    let opts = Opts::parse();

    let color_db = qql::color::ColorDb::from_bundle();
    let dt = qql::art::draw(opts.seed.as_bytes(), &color_db, &opts.config, opts.width).dt;
    let filename = format!(
        "{}{}.png",
        &opts.seed,
        if opts.config.inflate_draw_radius {
            "-inflated"
        } else {
            "-canon"
        }
    );
    dt.write_png(&filename).expect("dt.write_png");
    eprintln!("wrote png: {}", filename);
}
