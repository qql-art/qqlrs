use std::ffi::OsStr;
use std::fmt::{Debug, Display};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;

use qql::config::Animation;

#[derive(Parser)]
struct Opts {
    seed: Seed,

    /// Canvas width.
    ///
    /// This applies to the virtual canvas, before any viewport is computed. For instance, if
    /// `--width` is 1000 and `--viewport` is `0.5x0.5+0.25+0.25`, the actual output file will be
    /// 500px wide.
    #[clap(short, long, default_value = "2400")]
    width: i32,
    #[clap(short = 'o')]
    output_filename: Option<PathBuf>,
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
        let bytes: [u8; 32] = <[u8; 32]>::try_from(bytes)
            .map_err(|e| anyhow::anyhow!("Seed must be 32 bytes; got {}", e.len()))?;
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

    if let (Animation::None, true) = (&opts.config.animate, opts.config.splatter_immediately) {
        eprintln!("fatal: --splatter-immediately does not apply unless --animate is also set");
        std::process::exit(1);
    };

    let base_filepath = if let Some(f) = opts.output_filename {
        f
    } else {
        let mut basename = opts.seed.to_string();
        if opts.config.inflate_draw_radius {
            basename.push_str("-inflated");
        }
        if opts.config.fast_collisions {
            basename.push_str("-fastcoll");
        }
        basename.push_str(".png");
        PathBuf::from(basename)
    };

    let consume_frame = |frame: qql::art::Frame| {
        let filename = match frame.number {
            None => base_filepath.clone(),
            Some(n) => {
                let mut filename = base_filepath
                    .file_stem()
                    .unwrap_or(OsStr::new(""))
                    .to_owned();
                filename.push(format!("{:04}.", n));
                let mut filename = PathBuf::from(filename);
                if let Some(ext) = base_filepath.extension() {
                    filename.set_extension(ext);
                }
                base_filepath.with_file_name(filename)
            }
        };
        if let Err(e) = frame.dt.write_png(&filename) {
            eprintln!("Failed to write PNG to {}: {}", filename.display(), e);
            std::process::exit(1);
        }
        match frame.number {
            None => eprintln!("wrote png: {}", filename.display()),
            Some(n) => eprintln!("wrote frame {}: {}", n, filename.display()),
        };
    };

    let render_data = qql::art::draw(
        opts.seed.as_bytes(),
        &color_db,
        &opts.config,
        opts.width,
        consume_frame,
    );

    println!("num_points: {}", render_data.num_points);
    let color_names: Vec<&str> = render_data
        .colors_used
        .iter()
        .map(|k| {
            color_db
                .color(k)
                .map_or("<invalid color>", |c| c.name.as_str())
        })
        .collect();
    println!("colors: {:?}", color_names);
    println!("ring counts: {:?}", render_data.ring_counts_used);
}
