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
}
