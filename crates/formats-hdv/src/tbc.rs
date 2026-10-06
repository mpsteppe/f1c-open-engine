//! The tire brand `.tbc` file referenced by `[GENERAL] TireBrand`.
//!
//! The file is INI-like. `[SLIPCURVE]` holds named slip curves and their raw
//! `Data:` rows (ignored here). Every `[COMPOUND]` section is one tire compound:
//! `Name="..."` and `WetWeather=1`, followed by `Front:` and `Rear:` value
//! blocks. `Front:` and `Rear:` are free-text labels, not section headers, so
//! they are tracked by this parser to route each `Radius=` to the right block.
//! `TireCompoundSetting` in the `.hdv` is the compound index in file order.

use crate::ini::{parse_f64, parse_text, strip_comment};

/// Which value block of a `[COMPOUND]` a key belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Front,
    Rear,
}

/// One tire compound.
#[derive(Debug, Clone, PartialEq)]
pub struct Compound {
    /// `Name`, the display name such as `Hard Compound`.
    pub name: String,
    /// `WetWeather=1` when the compound is a rain tire.
    pub wet_weather: bool,
    /// `Front:` block `Radius`, the front tire radius in metres.
    pub front_radius: Option<f64>,
    /// `Rear:` block `Radius`, the rear tire radius in metres.
    pub rear_radius: Option<f64>,
}

/// Parsed tire brand file.
#[derive(Debug, Clone, Default)]
pub struct TbcFile {
    /// Every `[COMPOUND]` section, in file order.
    pub compounds: Vec<Compound>,
}

impl TbcFile {
    /// Parse the whole tire brand text.
    pub fn parse(text: &str) -> Self {
        let mut compounds: Vec<Compound> = Vec::new();
        let mut in_compound = false;
        let mut block: Option<Block> = None;

        for raw in text.lines() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = section_header(line) {
                in_compound = name.eq_ignore_ascii_case("COMPOUND");
                block = None;
                if in_compound {
                    compounds.push(Compound {
                        name: String::new(),
                        wet_weather: false,
                        front_radius: None,
                        rear_radius: None,
                    });
                }
                continue;
            }
            if !in_compound {
                continue;
            }
            if line.eq_ignore_ascii_case("Front:") {
                block = Some(Block::Front);
                continue;
            }
            if line.eq_ignore_ascii_case("Rear:") {
                block = Some(Block::Rear);
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            let Some(compound) = compounds.last_mut() else {
                continue;
            };
            if key.eq_ignore_ascii_case("Name") {
                compound.name = parse_text(value);
            } else if key.eq_ignore_ascii_case("WetWeather") {
                compound.wet_weather = value == "1";
            } else if key.eq_ignore_ascii_case("Radius") {
                match block {
                    Some(Block::Front) => compound.front_radius = parse_f64(value),
                    Some(Block::Rear) => compound.rear_radius = parse_f64(value),
                    None => {}
                }
            }
        }
        TbcFile { compounds }
    }

    /// The compound at an index, or `None` when the index is out of range.
    pub fn compound(&self, index: usize) -> Option<&Compound> {
        self.compounds.get(index)
    }

    /// Name of the compound at an index.
    pub fn compound_name(&self, index: usize) -> Option<&str> {
        self.compound(index).map(|compound| compound.name.as_str())
    }

    /// `(front, rear)` tire radius of the compound at an index, when both are
    /// present and finite.
    pub fn compound_radii(&self, index: usize) -> Option<(f64, f64)> {
        let compound = self.compound(index)?;
        let front = compound.front_radius.filter(|value| value.is_finite())?;
        let rear = compound.rear_radius.filter(|value| value.is_finite())?;
        Some((front, rear))
    }
}

/// The name inside a `[NAME]` line, or `None` when the line is not one.
fn section_header(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']').map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TBC: &str = "\
[SLIPCURVE]
Name=\"Default\"
Data:
0.000000 0.100000 0.200000

[COMPOUND]
Name=\"Prime Compound\"
WetWeather=1
Front:
DryLatLong=(1.500, 1.600)
Radius=0.300
Rear:
Radius=0.310

[COMPOUND]
Name=\"Option Compound\"
WetWeather=1
Front:
Radius=0.300
";

    #[test]
    fn reads_every_compound_in_order() {
        let tbc = TbcFile::parse(TBC);
        assert_eq!(tbc.compounds.len(), 2);
        assert_eq!(tbc.compound_name(0), Some("Prime Compound"));
        assert_eq!(tbc.compound_name(1), Some("Option Compound"));
    }

    #[test]
    fn reads_wet_weather_and_out_of_range() {
        let tbc = TbcFile::parse(TBC);
        assert!(tbc.compound(0).unwrap().wet_weather);
        assert_eq!(tbc.compound_name(2), None);
    }

    #[test]
    fn separates_front_and_rear_radius() {
        let tbc = TbcFile::parse(TBC);
        assert_eq!(tbc.compound(0).unwrap().front_radius, Some(0.300));
        assert_eq!(tbc.compound(0).unwrap().rear_radius, Some(0.310));
        assert_eq!(tbc.compound(1).unwrap().front_radius, Some(0.300));
        assert_eq!(tbc.compound(1).unwrap().rear_radius, None);
        assert_eq!(tbc.compound_radii(0), Some((0.300, 0.310)));
        assert_eq!(tbc.compound_radii(1), None);
    }

    #[test]
    fn radius_only_counts_inside_a_block() {
        let tbc = TbcFile::parse("[COMPOUND]\nName=\"X\"\nRadius=0.5\nFront:\nRadius=0.4\n");
        assert_eq!(tbc.compound(0).unwrap().front_radius, Some(0.4));
        assert_eq!(tbc.compound(0).unwrap().rear_radius, None);
    }
}
