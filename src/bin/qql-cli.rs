use hex_literal::hex;

fn main() {
    let color_db = qql::color::ColorDb::from_bundle();

    // QQL #245, orbital layout
    let seed = hex!("e03a5189dac8182085e4adf66281f679fff2291df504077c1df9ee957112414d");
    qql::art::draw(&seed, &color_db);
    println!();

    // QQL #4, shadows layout
    let seed = hex!("dde2e1aa38442089bdb28be76009f42c65df45db0a4e2140495effff10a0b2d5");
    qql::art::draw(&seed, &color_db);
    println!();

    // QQL #18, formation layout
    let seed = hex!("d8478053a45bfcdc7a8411b27c7329274c49de05058a753fc5d5ffff10e50acd");
    qql::art::draw(&seed, &color_db);
    println!();
}
