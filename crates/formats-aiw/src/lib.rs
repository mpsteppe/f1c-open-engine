//! Parser for the gMotor `.aiw` track description, grid section only.
//!
//! The file is plain text (Latin-1); the caller decodes the bytes and hands the
//! parser a `&str`. It is INI-like: a section starts at a `[NAME]` line and
//! runs until the next `[NAME]` line. Only `[GRID]` is read here; every other
//! section is ignored. Keys are case-insensitive.
//!
//! `[GRID]` holds repeated triples:
//! `GridIndex=0` / `Pos=(x,y,z)` / `Ori=(x,y,z)`. `Pos` is metres in the same
//! game axes as MTS vertices, `Ori` is three angles in radians around X, Y, Z.

/// One grid slot of a track's `.aiw` file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSlot {
    /// The `GridIndex=` value.
    pub index: u32,
    /// The `Pos=` value (metres, game axes).
    pub pos: [f32; 3],
    /// The `Ori=` value (radians around X, Y, Z); zero when absent.
    pub ori: [f32; 3],
}

/// Every `[GRID]` slot of a `.aiw` file, in file order.
///
/// A slot starts at each `GridIndex=` line. `Pos=` and `Ori=` assign to the
/// most recent slot; an absent `Ori=` stays `[0.0, 0.0, 0.0]`. Content outside
/// `[GRID]` is ignored. `Pos=`/`Ori=` before the first `GridIndex=` are dropped.
pub fn grid_slots(text: &str) -> Vec<GridSlot> {
    let mut slots: Vec<GridSlot> = Vec::new();
    let mut in_grid = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(section) = section_name(line) {
            in_grid = section.eq_ignore_ascii_case("grid");
            continue;
        }
        if !in_grid {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.eq_ignore_ascii_case("gridindex") {
            let index = value.trim().parse().unwrap_or(slots.len() as u32);
            slots.push(GridSlot {
                index,
                pos: [0.0; 3],
                ori: [0.0; 3],
            });
        } else if key.eq_ignore_ascii_case("pos") {
            if let (Some(slot), Some(pos)) = (slots.last_mut(), parse_tuple(value)) {
                slot.pos = pos;
            }
        } else if key.eq_ignore_ascii_case("ori") {
            if let (Some(slot), Some(ori)) = (slots.last_mut(), parse_tuple(value)) {
                slot.ori = ori;
            }
        }
    }

    slots
}

/// The name inside a leading `[NAME]` line, if the line is one.
fn section_name(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']')
}

/// Parse a `(x, y, z)` value.
fn parse_tuple(value: &str) -> Option<[f32; 3]> {
    let inner = value.trim().strip_prefix('(')?.strip_suffix(')')?;
    let mut parts = inner.split(',');
    let x = parts.next()?.trim().parse().ok()?;
    let y = parts.next()?.trim().parse().ok()?;
    let z = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([x, y, z])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[Features]
SomeKey=SomeValue

[GRID]
GridIndex=0
Pos=(12.375,2.125,-45.625)
Ori=(-0.021,1.125,-0.031)
GridIndex=1
Pos=(15.875,2.375,-49.125)
Ori=(-0.041,1.375,0.001)

[PITS]
TeamIndex=0
PitPos=(22.625,2.625,-38.875)
PitOri=(-0.041,1.625,-0.031)
";

    #[test]
    fn parses_inline_sample() {
        let slots = grid_slots(SAMPLE);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].index, 0);
        assert_eq!(slots[0].pos, [12.375, 2.125, -45.625]);
        assert_eq!(slots[0].ori, [-0.021, 1.125, -0.031]);
        assert_eq!(slots[1].index, 1);
        assert_eq!(slots[1].pos, [15.875, 2.375, -49.125]);
        assert_eq!(slots[1].ori, [-0.041, 1.375, 0.001]);
    }

    #[test]
    fn tolerates_extra_spacing() {
        let text =
            "[GRID]\n  GridIndex = 5 \n Pos = ( 1.5 , 2.5 , 3.5 ) \n Ori = ( 0.1 , 0.2 , 0.3 ) \n";
        let slots = grid_slots(text);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].index, 5);
        assert_eq!(slots[0].pos, [1.5, 2.5, 3.5]);
        assert_eq!(slots[0].ori, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn missing_ori_defaults_to_zero() {
        let text = "[GRID]\nGridIndex=3\nPos=(10.0,20.0,30.0)\n";
        let slots = grid_slots(text);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].ori, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn other_sections_are_ignored() {
        let slots = grid_slots(SAMPLE);
        assert_eq!(slots.len(), 2);
        assert!(slots.iter().all(|slot| slot.index <= 1));
    }

    #[test]
    fn keys_and_section_are_case_insensitive() {
        let text = "[grid]\nGRIDINDEX=7\npos=(1.0,2.0,3.0)\nORI=(0.25,0.5,0.75)\n";
        let slots = grid_slots(text);
        assert_eq!(
            slots,
            vec![GridSlot {
                index: 7,
                pos: [1.0, 2.0, 3.0],
                ori: [0.25, 0.5, 0.75],
            }]
        );
    }

    #[test]
    fn pos_and_ori_before_first_index_are_dropped() {
        let text = "[GRID]\nPos=(9.0,9.0,9.0)\nOri=(1.0,1.0,1.0)\n";
        assert!(grid_slots(text).is_empty());
    }

    #[test]
    fn empty_and_absent_grid_are_empty() {
        assert!(grid_slots("").is_empty());
        assert!(grid_slots("[PITS]\nPitPos=(1.0,2.0,3.0)\n").is_empty());
    }
}
