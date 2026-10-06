//! The suspension physical-model `.pm` file referenced by
//! `[SUSPENSION] PhysicalModelFile`.
//!
//! Unlike the `.hdv`, a `.pm` line may hold several `key=value` pairs separated
//! by spaces, so this module reads the pairs itself instead of using the shared
//! INI parser. A value that starts with `(` is kept whole up to its matching
//! `)`, so a tuple such as `pos=(1, 2, 3)` survives its internal spaces.
//! `[BODY]` sections describe rigid masses; `[BAR]`, `[JOINT]`, `[HINGE]` and
//! `[JOINT&HINGE]` sections are constraints. Only the bodies and the constraint
//! counts are decoded, which is enough to prove the file loads.

use crate::ini::{parse_tuple, parse_tuple3, strip_comment};

/// One `[BODY]` rigid mass.
#[derive(Debug, Clone, PartialEq)]
pub struct PmBody {
    pub name: String,
    pub mass: Option<f64>,
    /// Inertia in kg m^2.
    pub inertia: Option<[f64; 3]>,
    /// Position in metres, game axes.
    pub pos: Option<[f64; 3]>,
    /// Orientation in radians around X, Y, Z.
    pub ori: Option<[f64; 3]>,
}

/// Parsed physical-model file.
#[derive(Debug, Clone, Default)]
pub struct PmFile {
    /// Every `[BODY]` in file order.
    pub bodies: Vec<PmBody>,
    /// Number of `[BAR]` constraints.
    pub bars: usize,
    /// Number of `[JOINT]` constraints.
    pub joints: usize,
    /// Number of `[HINGE]` constraints.
    pub hinges: usize,
    /// Number of `[JOINT&HINGE]` constraints.
    pub joint_hinges: usize,
}

impl PmFile {
    /// Parse the whole `.pm` text.
    pub fn parse(text: &str) -> Self {
        let mut file = PmFile::default();
        let mut in_body = false;

        for raw in text.lines() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(section) = header(line) {
                in_body = false;
                match section.to_ascii_uppercase().as_str() {
                    "BODY" => {
                        file.bodies.push(PmBody {
                            name: String::new(),
                            mass: None,
                            inertia: None,
                            pos: None,
                            ori: None,
                        });
                        in_body = true;
                    }
                    "BAR" => file.bars += 1,
                    "JOINT" => file.joints += 1,
                    "HINGE" => file.hinges += 1,
                    "JOINT&HINGE" => file.joint_hinges += 1,
                    _ => {}
                }
                continue;
            }
            if !in_body {
                continue;
            }
            for (key, value) in split_pairs(line) {
                apply_body(&mut file.bodies, &key, &value);
            }
        }
        file
    }

    /// Look up a body by name (case-insensitive).
    pub fn body(&self, name: &str) -> Option<&PmBody> {
        self.bodies
            .iter()
            .find(|body| body.name.eq_ignore_ascii_case(name))
    }
}

/// Apply one `key=value` token to the last `[BODY]`.
fn apply_body(bodies: &mut [PmBody], key: &str, value: &str) {
    let Some(body) = bodies.last_mut() else {
        return;
    };
    match key.to_ascii_lowercase().as_str() {
        "name" => body.name = value.to_string(),
        "mass" => body.mass = parse_tuple(value).and_then(|values| values.first().copied()),
        "inertia" => body.inertia = parse_tuple3(value),
        "pos" => body.pos = parse_tuple3(value),
        "ori" => body.ori = parse_tuple3(value),
        _ => {}
    }
}

/// Split one line into `key=value` pairs.
///
/// A value that starts with `(` runs to the matching `)`, keeping any spaces
/// inside the tuple. Every other value runs to the next whitespace. Tokens
/// without an `=` are skipped.
fn split_pairs(line: &str) -> Vec<(String, String)> {
    let bytes = line.as_bytes();
    let mut pairs = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }

        let key_start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'=' && !bytes[cursor].is_ascii_whitespace()
        {
            cursor += 1;
        }
        if cursor >= bytes.len() || bytes[cursor] != b'=' {
            while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            continue;
        }
        let key = &line[key_start..cursor];

        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let value_start = cursor;
        if cursor < bytes.len() && bytes[cursor] == b'(' {
            let mut depth = 0;
            while cursor < bytes.len() {
                match bytes[cursor] {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        cursor += 1;
                        if depth == 0 {
                            break;
                        }
                        continue;
                    }
                    _ => {}
                }
                cursor += 1;
            }
        } else {
            while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
        }
        pairs.push((key.to_string(), line[value_start..cursor].to_string()));
    }

    pairs
}

/// The name inside a `[NAME]` line, or `None`.
fn header(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']').map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PM: &str = "\
[BODY]
name=cabin mass=(9) inertia=(0.1,0.2,0.3)
pos=(0.0,0.0,0.0) ori=(0.0,0.0,0.0)

[BODY]
name=chassis mass=(320.0) inertia=(100.0,150.0,20.0)
pos=(0.0,0.0,0.0) ori=(0.0,0.0,0.0)

[JOINT&HINGE]
posbody=hub negbody=upright pos=hub axis=(-1.00,0.0,0.0)

[BAR]
posbody=chassis negbody=upright pos=(1, 2, 3) neg=(4, 5, 6)

[BAR]
posbody=chassis negbody=upright pos=(2, 3, 4) neg=(5, 6, 7)

[HINGE]
posbody=a negbody=b
";

    #[test]
    fn reads_bodies_with_fields() {
        let pm = PmFile::parse(PM);
        assert_eq!(pm.bodies.len(), 2);
        assert_eq!(pm.body("chassis").unwrap().mass, Some(320.0));
        assert_eq!(pm.body("cabin").unwrap().mass, Some(9.0));
        assert_eq!(
            pm.body("chassis").unwrap().inertia,
            Some([100.0, 150.0, 20.0])
        );
    }

    #[test]
    fn keeps_spaced_tuple_values() {
        let pm = PmFile::parse(
            "[BODY]\nname=sample mass=(12) inertia=(1, 2, 3) pos=(4, 5, 6) ori=(7, 8, 9)\n",
        );
        let body = pm.body("sample").unwrap();
        assert_eq!(body.mass, Some(12.0));
        assert_eq!(body.inertia, Some([1.0, 2.0, 3.0]));
        assert_eq!(body.pos, Some([4.0, 5.0, 6.0]));
        assert_eq!(body.ori, Some([7.0, 8.0, 9.0]));
    }

    #[test]
    fn spaced_and_tight_pairs_parse_the_same() {
        let spaced = PmFile::parse("[BODY]\nname=x mass=(5) pos=(1, 2, 3)\n");
        let tight = PmFile::parse("[BODY]\nname=x mass=(5) pos=(1,2,3)\n");
        assert_eq!(spaced.bodies, tight.bodies);
    }

    #[test]
    fn counts_constraints() {
        let pm = PmFile::parse(PM);
        assert_eq!(pm.bars, 2);
        assert_eq!(pm.joint_hinges, 1);
        assert_eq!(pm.hinges, 1);
        assert_eq!(pm.joints, 0);
    }

    #[test]
    fn comments_are_ignored() {
        let pm = PmFile::parse("[BODY]\nname=body mass=(1) // the chassis\n// [BAR]\n");
        assert_eq!(pm.bodies.len(), 1);
        assert_eq!(pm.bars, 0);
    }

    #[test]
    fn first_mass_number_is_used() {
        let pm = PmFile::parse("[BODY]\nname=x mass=(10,20)\n");
        assert_eq!(pm.bodies[0].mass, Some(10.0));
    }
}
