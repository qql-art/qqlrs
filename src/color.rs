use std::collections::{hash_map::Entry::*, BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

const COLORS_JSON: &str = include_str!("colordata.json");

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorSpec {
    pub name: String,
    pub hue_variance: f64,
    pub sat_variance: f64,
    pub bright_variance: f64,
    pub hue: f64,
    pub hue_min: f64,
    pub hue_max: f64,
    pub sat: f64,
    pub sat_min: f64,
    pub sat_max: f64,
    pub bright: f64,
    pub bright_min: f64,
    pub bright_max: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WirePaletteSpec {
    name: String,
    swatches: Vec<String>,
    color_seq: Vec<String>,
    background_colors: Vec<WireBackgroundColorSpec>,
    splatter_colors: Vec<WireSplatterColorSpec>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireBackgroundColorSpec(
    /// Name.
    String,
    /// Weight.
    f64,
    /// Substitutions.
    BTreeMap<String, Option<String>>,
);

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireSplatterColorSpec(
    /// Name.
    String,
    /// Weight.
    f64,
);

#[derive(Debug, Deserialize, Serialize)]
pub struct WireColorDb {
    colors: Vec<ColorSpec>,
    palettes: Vec<WirePaletteSpec>,
}

pub type ColorKey = u32;

#[derive(Debug, Serialize)]
pub struct PaletteSpec {
    pub swatches: Vec<u32>,
    pub color_seq: Vec<u32>,
    pub background_colors: Vec<(BackgroundColorSpec, f64)>,
    pub splatter_colors: Vec<(ColorKey, f64)>,
}

#[derive(Debug, Serialize)]
pub struct BackgroundColorSpec {
    pub color: ColorKey,
    pub substitutions: HashMap<ColorKey, Option<ColorKey>>,
}

#[derive(Debug, Serialize)]
pub struct ColorDb {
    colors: Vec<ColorSpec>,
    colors_by_name: HashMap<String, ColorKey>,
    palettes: HashMap<String, PaletteSpec>,
}

#[derive(Debug)]
pub enum WireFormatError {
    TooManyColors,
    DuplicateColor { name: String },
    DuplicatePalette { name: String },
    UndefinedColor { name: String, palette: String },
}

impl ColorDb {
    pub fn from_bundle() -> Self {
        let wire: WireColorDb =
            serde_json::from_str(COLORS_JSON).expect("bundled data is invalid JSON");
        ColorDb::from_wire(wire).expect("bundled data is not a valid database")
    }

    pub fn from_wire(wire: WireColorDb) -> Result<Self, WireFormatError> {
        let mut db = ColorDb {
            colors: Vec::with_capacity(wire.colors.len()),
            colors_by_name: HashMap::with_capacity(wire.colors.len()),
            palettes: HashMap::with_capacity(wire.palettes.len()),
        };

        for color in wire.colors {
            let color_key =
                ColorKey::try_from(db.colors.len()).map_err(|_| WireFormatError::TooManyColors)?;
            let name = color.name.clone();
            db.colors.push(color);
            match db.colors_by_name.entry(name) {
                Occupied(o) => {
                    let name = o.remove_entry().0;
                    return Err(WireFormatError::DuplicateColor { name });
                }
                Vacant(v) => v.insert(color_key),
            };
        }

        for palette in wire.palettes {
            let palette_entry = match db.palettes.entry(palette.name) {
                Occupied(o) => {
                    let name = o.remove_entry().0;
                    return Err(WireFormatError::DuplicatePalette { name });
                }
                Vacant(v) => v,
            };
            let find = |color: String| -> Result<ColorKey, WireFormatError> {
                Self::look_up(color, palette_entry.key(), &db.colors_by_name)
            };
            let swatches = palette
                .swatches
                .into_iter()
                .map(find)
                .collect::<Result<Vec<ColorKey>, WireFormatError>>()?;
            let color_seq = palette
                .color_seq
                .into_iter()
                .map(find)
                .collect::<Result<Vec<ColorKey>, WireFormatError>>()?;
            let background_colors = palette
                .background_colors
                .into_iter()
                .map(|WireBackgroundColorSpec(bg, weight, substitutions)| {
                    let substitutions = substitutions
                        .into_iter()
                        .map(|(from, maybe_to)| {
                            let from: ColorKey = find(from)?;
                            let maybe_to = maybe_to.map(find).transpose()?;
                            Ok((from, maybe_to))
                        })
                        .collect::<Result<HashMap<ColorKey, Option<ColorKey>>, WireFormatError>>(
                        )?;
                    let spec = BackgroundColorSpec {
                        color: find(bg)?,
                        substitutions,
                    };
                    Ok((spec, weight))
                })
                .collect::<Result<Vec<(BackgroundColorSpec, f64)>, WireFormatError>>()?;
            let splatter_colors = palette
                .splatter_colors
                .into_iter()
                .map(|WireSplatterColorSpec(c, weight)| Ok((find(c)?, weight)))
                .collect::<Result<Vec<(ColorKey, f64)>, WireFormatError>>()?;
            palette_entry.insert(PaletteSpec {
                swatches,
                color_seq,
                background_colors,
                splatter_colors,
            });
        }

        Ok(db)
    }

    fn look_up(
        name: String,
        palette: &str,
        map: &HashMap<String, ColorKey>,
    ) -> Result<ColorKey, WireFormatError> {
        match map.get(&name) {
            Some(v) => Ok(*v),
            None => Err(WireFormatError::UndefinedColor {
                name,
                palette: palette.into(),
            }),
        }
    }

    pub fn color(&self, key: ColorKey) -> Option<&ColorSpec> {
        self.colors.get(key as usize)
    }

    pub fn color_by_name(&self, name: &str) -> Option<&ColorSpec> {
        self.color(*self.colors_by_name.get(name)?)
    }

    pub fn palette(&self, palette_key: crate::traits::ColorPalette) -> Option<&PaletteSpec> {
        use crate::traits::ColorPalette::*;
        let name = match palette_key {
            Austin => "austin",
            Berlin => "berlin",
            Edinburgh => "edinburgh",
            Fidenza => "fidenza",
            Miami => "miami",
            Seattle => "seattle",
            Seoul => "seoul",
        };
        self.palettes.get(name)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_color_db_from_bundle() {
        let db = ColorDb::from_bundle();
        assert!(!db.colors.is_empty());
        assert!(!db.palettes.is_empty());
    }
}
