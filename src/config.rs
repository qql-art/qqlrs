use std::{fmt::Display, num::NonZeroU32, str::FromStr};

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
    #[clap(long, value_name = "STEPS")]
    pub min_circle_steps: Option<u32>,

    /// Restrict rendering to a region of the canvas.
    ///
    /// Values are specified as floats from 0.0 (top/left) to 1.0 (bottom/right). For instance,
    /// `0.1x0.1+0.45+0.45` renders the center 1% of the canvas. See `--width` about how this
    /// affects the output image size.
    #[clap(long, value_name = "WxH+X+Y")]
    pub viewport: Option<FractionalViewport>,

    /// Chunks for parallel rendering.
    #[clap(long, value_name = "WxH", default_value_t)]
    pub chunks: Chunks,

    /// Output multiple frames showing the construction of the piece.
    ///
    /// May be `none` for no animation, `groups` to paint one flow line group at a time, or
    /// `points:N` (where `N` is a positive integer) to show paint `N` points at a time.
    #[clap(long, default_value_t)]
    pub animate: Animation,

    /// Animate in splatter points immediately after their parents.
    ///
    /// By default, all splatters are deferred to the end of the animation. With this option,
    /// each splatter point is instead drawn immediately after the point that spawned it. This
    /// takes additional processing and may increase render time. Can only be used if `--animate`
    /// is also set.
    #[clap(long, default_value_t)]
    pub splatter_immediately: bool,
}

#[derive(Default, Debug, Clone)]
pub enum Animation {
    #[default]
    None,
    Groups,
    Points {
        step: u32,
    },
}

impl FromStr for Animation {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(n) = s.strip_prefix("points:") {
            let step: u32 = n.parse().map_err(|e| {
                anyhow::anyhow!("Invalid number of points per frame {:?}: {}", n, e)
            })?;
            if step == 0 {
                anyhow::bail!("Must add at least 1 point per frame");
            }
            return Ok(Animation::Points { step });
        }
        match s {
            "none" => Ok(Animation::None),
            "groups" => Ok(Animation::Groups),
            "points" => {
                anyhow::bail!("Must specify how many points to add per frame, like \"points:100\"")
            }
            _ => anyhow::bail!("Invalid animation spec"),
        }
    }
}

impl Display for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Animation::None => f.write_str("none"),
            Animation::Groups => f.write_str("groups"),
            Animation::Points { step } => write!(f, "points:{}", step),
        }
    }
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
    pub fn from_whlt(width: f64, height: f64, left: f64, top: f64) -> Self {
        Self {
            width,
            height,
            left,
            top,
        }
    }
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

#[derive(Debug, PartialEq, Clone)]
pub struct Chunks {
    pub w: NonZeroU32,
    pub h: NonZeroU32,
}

impl FromStr for Chunks {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (w, h) = s.split_once('x').context("Invalid format; expected WxH")?;
        let w: u32 = w.parse().context("Invalid width")?;
        let h: u32 = h.parse().context("Invalid height")?;
        let w = NonZeroU32::try_from(w).context("Chunk width cannot be zero")?;
        let h = NonZeroU32::try_from(h).context("Chunk height cannot be zero")?;
        Ok(Chunks { w, h })
    }
}

impl Default for Chunks {
    fn default() -> Self {
        let one = NonZeroU32::new(1).unwrap();
        Chunks { w: one, h: one }
    }
}

impl Display for Chunks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.w, self.h)
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
