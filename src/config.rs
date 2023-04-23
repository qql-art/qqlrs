#[derive(Debug, Default, clap::Args)]
pub struct Config {
    /// Speed up collision checking by avoiding our slow `sqrt` implementation. May slightly
    /// affect layout.
    #[clap(long)]
    pub fast_collisions: bool,

    /// At paint time, ensure that all points have at least a small positive radius.
    #[clap(long)]
    pub inflate_draw_radius: bool,
}
