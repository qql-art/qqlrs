use qql::rand::Rng;

fn main() {
    let mut rng = Rng::from_seed(b"");
    for _ in 0..10_000_000 {
        rng.gauss(0.0, 1.0);
    }
    println!("{}", rng.gauss(0.0, 1.0));
}
