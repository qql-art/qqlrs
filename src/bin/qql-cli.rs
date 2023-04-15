use hex_literal::hex;

fn main() {
    let color_db = qql::color::ColorDb::from_bundle();
    let seed = hex!("e03a5189dac8182085e4adf66281f679fff2291df504077c1df9ee957112414d");
    qql::art::draw(&seed, &color_db);
}
