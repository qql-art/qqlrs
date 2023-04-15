use hex_literal::hex;

use qql::color;

fn main() {
    let wire: color::WireColorDb = serde_json::from_str(color::COLORS_JSON).expect("parse failed");
    let color_db = color::ColorDb::from_wire(wire).expect("build failed");

    let seed = hex!("e03a5189dac8182085e4adf66281f679fff2291df504077c1df9ee957112414d");
    qql::art::draw(&seed, &color_db);
}
