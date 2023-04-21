use std::num::ParseIntError;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!(
            "usage: {} <seed>",
            args.get(0).map(String::as_str).unwrap_or("qql-cli")
        );
        std::process::exit(1);
    }
    let seed: [u8; 32] = match decode_hex(args[1].trim_start_matches("0x"))
        .ok()
        .and_then(|s| TryFrom::try_from(s).ok())
    {
        Some(x) => x,
        _ => {
            eprintln!("invalid seed");
            std::process::exit(1);
        }
    };

    let color_db = qql::color::ColorDb::from_bundle();
    let points = qql::art::draw(&seed, &color_db);
    let mut stdout = std::io::stdout().lock();
    let hsb_to_arr = |hsb: qql::art::Hsb| serde_json::json!([hsb.0, hsb.1, hsb.2, 100]);
    for pt in points.0 {
        let json = serde_json::json!({
            "point": [pt.position.0, pt.position.1],
            "scale": pt.scale,
            "color": hsb_to_arr(pt.primary_color),
            "secondaryColor": hsb_to_arr(pt.secondary_color),
            "bullseyeSpec": {
                "rings": pt.bullseye.rings,
                "density": pt.bullseye.density,
            },
        });
        serde_json::to_writer(&mut stdout, &json).unwrap();
    }
}

// copied from Sven Marnach: <https://stackoverflow.com/a/52992629>
fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}
