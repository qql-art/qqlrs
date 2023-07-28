use std::str::FromStr;

use anyhow::Context;

#[derive(Debug, Default, clap::Args)]
pub struct Config {
    /// Speed up collision checking by avoiding our slow `sqrt` implementation. May slightly
    /// affect layout.
    #[clap(long)]
    pub fast_collisions: bool,

    /// At paint time, ensure that all points have at least a small positive radius.
    #[clap(long)]
    pub inflate_draw_radius: bool,

    /// Use at least this many segments for every circle. Values below `8` have no effect.
    ///
    /// At typical resolutions, circles should look smooth without tweaking. But at very large
    /// resolutions (say, above 10k pixels wide), the segments may start to become visible,
    /// especially on small circles. Crank this value up linearly to compensate, at the cost of
    /// render time.
    #[clap(long)]
    pub min_circle_steps: Option<u32>,

    /// Restrict rendering to a region of the canvas.
    ///
    /// Values are specified as floats from 0.0 (top/left) to 1.0 (bottom/right). For instance,
    /// `0.1x0.1+0.45+0.45` renders the center 1% of the canvas. See `--width` about how this
    /// affects the output image size.
    #[clap(long, value_name = "WxH+X+Y")]
    pub viewport: Option<FractionalViewport>,
}

/// A viewport/crop specification in fractional space, where both axes range from `0.0` to `1.0`.
#[derive(Debug, PartialEq, Clone)]
pub struct FractionalViewport {
    width: f64,
    height: f64,
    left: f64,
    top: f64,
}

impl Default for FractionalViewport {
    fn default() -> Self {
        Self {
            width: 1.0,
            height: 1.0,
            left: 0.0,
            top: 0.0,
        }
    }
}

impl FractionalViewport {
    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn height(&self) -> f64 {
        self.height
    }
    pub fn left(&self) -> f64 {
        self.left
    }
    pub fn top(&self) -> f64 {
        self.top
    }
    pub fn right(&self) -> f64 {
        self.left + self.width
    }
    pub fn bottom(&self) -> f64 {
        self.top + self.height
    }
}

/// Expects a string like `WxH+X+Y`, as with imagemagick geometry syntax.
impl FromStr for FractionalViewport {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn parts(s: &str) -> Option<(&str, &str, &str, &str)> {
            let (width, s) = s.split_once('x')?;
            let (height, s) = s.split_once('+')?;
            let (left, top) = s.split_once('+')?;
            Some((width, height, left, top))
        }
        let (width, height, left, top) = parts(s).context("Invalid format; expected WxH+X+Y")?;
        Ok(FractionalViewport {
            width: width.parse().context("Invalid width")?,
            height: height.parse().context("Invalid height")?,
            left: left.parse().context("Invalid x-offset")?,
            top: top.parse().context("Invalid y-offset")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_fromstr_ok() {
        assert_eq!(
            "100x200+30+60".parse::<FractionalViewport>().unwrap(),
            FractionalViewport {
                width: 100.0,
                height: 200.0,
                left: 30.0,
                top: 60.0
            }
        );
    }

    #[test]
    fn test_viewport_fromstr_errs() {
        fn check(input: &str, expected_err: &str) {
            let msg = input.parse::<FractionalViewport>().unwrap_err().to_string();
            assert_eq!(msg, expected_err);
        }
        check("100x200+30", "Invalid format; expected WxH+X+Y");
        check("FOOx200+30+60", "Invalid width");
        check("100xBAR+30+60", "Invalid height");
        check("100x200+BAZ+60", "Invalid x-offset");
        check("100x200+30+QUUX", "Invalid y-offset");
    }
}
