use std::time::Instant;

use qql::color;

fn main() {
    let wire = {
        let start = Instant::now();
        let wire: color::WireColorDb =
            serde_json::from_str(color::COLORS_JSON).expect("parse failed");
        eprintln!("parsed JSON from string in {:?}", start.elapsed());
        wire
    };
    let start = Instant::now();
    let db = color::ColorDb::from_wire(wire).expect("build failed");
    eprintln!("built database in {:?}", start.elapsed());
    dbg!(db);
}
