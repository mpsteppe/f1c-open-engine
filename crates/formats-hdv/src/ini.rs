//! Minimal INI-like parser shared by the gMotor physics text files.
//!
//! A file is a list of lines. `//` starts a comment that runs to the end of the
//! line. A line `[NAME]` starts a new section; any other line that contains an
//! `=` is a `key=value` pair (the first `=` splits the two, both sides are
//! trimmed). Lines that are neither a section header nor a `key=value` pair are
//! ignored, which lets this parser read the data lines of `.tbc` curves.
//!
//! Keys and section names are compared case-insensitively. Every entry is kept,
//! so a key may appear many times (for example the repeated `ratio=` lines of a
//! gear file); callers choose the first value or iterate all of them.

/// One `key=value` pair, both trimmed, the comment already removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: String,
}

/// One `[NAME]` section and the entries that follow it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    /// Section name without the brackets; empty for entries before any header.
    pub name: String,
    pub entries: Vec<Entry>,
}

impl Section {
    /// Value of the first `key=` entry, or `None` when the key is absent.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.key.eq_ignore_ascii_case(key))
            .map(|entry| entry.value.as_str())
    }

    /// Values of every `key=` entry, in file order.
    pub fn values<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.entries
            .iter()
            .filter(move |entry| entry.key.eq_ignore_ascii_case(key))
            .map(|entry| entry.value.as_str())
    }

    /// Resolve a `XRange=(min, step, count)` / `XSetting=n` pair.
    ///
    /// `base` is the key without the `Range` / `Setting` suffix, for example
    /// `"RevLimit"` for `RevLimitRange` and `RevLimitSetting`. The value is
    /// `min + step * setting`. `None` when either key is absent or malformed.
    pub fn ranged(&self, base: &str) -> Option<Ranged> {
        let range = parse_tuple(self.value(&format!("{base}Range"))?)?;
        let setting = parse_f64(self.value(&format!("{base}Setting"))?)?;
        if range.len() < 2 {
            return None;
        }
        Some(Ranged {
            min: range[0],
            step: range[1],
            count: range.get(2).copied().unwrap_or(0.0),
            setting,
            value: range[0] + range[1] * setting,
        })
    }
}

/// A `XRange` / `XSetting` pair with the value already computed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ranged {
    pub min: f64,
    pub step: f64,
    pub count: f64,
    pub setting: f64,
    pub value: f64,
}

/// A parsed INI-like file: sections in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ini {
    pub sections: Vec<Section>,
}

impl Ini {
    /// Parse the whole text. See the module documentation for the rules.
    pub fn parse(text: &str) -> Self {
        let mut sections: Vec<Section> = Vec::new();
        let mut current: Option<Section> = None;

        for raw in text.lines() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = section_header(line) {
                if let Some(section) = current.take() {
                    sections.push(section);
                }
                current = Some(Section {
                    name: name.to_string(),
                    entries: Vec::new(),
                });
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                let section = current.get_or_insert_with(Section::default);
                section.entries.push(Entry {
                    key: key.to_string(),
                    value: value.trim().to_string(),
                });
            }
        }
        if let Some(section) = current {
            sections.push(section);
        }
        Ini { sections }
    }

    /// First section whose name matches `name` (case-insensitive).
    ///
    /// Use the empty string for the entries that appear before any header, such
    /// as the whole body of an engine file.
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections
            .iter()
            .find(|section| section.name.eq_ignore_ascii_case(name))
    }

    /// Every section whose name matches `name` (case-insensitive), in order.
    pub fn sections_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Section> + 'a {
        self.sections
            .iter()
            .filter(move |section| section.name.eq_ignore_ascii_case(name))
    }

    /// Value of the first `key=` entry of the first matching section.
    pub fn value(&self, section: &str, key: &str) -> Option<&str> {
        self.section(section)?.value(key)
    }

    /// Values of every `key=` entry of the first matching section, in order.
    pub fn values<'a>(
        &'a self,
        section: &'a str,
        key: &'a str,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.section(section)
            .into_iter()
            .flat_map(move |section| section.values(key))
    }
}

/// Text before the first `//`, or the whole line when there is none.
pub(crate) fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(index) => &line[..index],
        None => line,
    }
}

/// The name inside a `[NAME]` line, or `None` when the line is not one.
fn section_header(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']').map(str::trim)
}

/// Parse a single number, allowing leading/trailing spaces.
pub fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse().ok()
}

/// Parse a non-negative index from a number that may be written as `4.0`.
///
/// Rejects NaN, infinity, negative and fractional values, and any value that
/// would not fit in a `usize`. The upper bound is compared as an `f64` before
/// the cast, so no saturating conversion can turn an overflow into a valid
/// index (`usize::MAX as f64` rounds up to a value no representable whole
/// `f64` can equal without being out of range).
pub fn parse_usize(value: &str) -> Option<usize> {
    let number = parse_f64(value)?;
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 {
        return None;
    }
    if number >= usize::MAX as f64 {
        return None;
    }
    Some(number as usize)
}

/// Parse a `(a, b, c)` tuple into its number list. Allows spaces.
pub fn parse_tuple(value: &str) -> Option<Vec<f64>> {
    let inner = value.trim().strip_prefix('(')?.strip_suffix(')')?;
    inner.split(',').map(parse_f64).collect()
}

/// Parse a `(a, b, c)` tuple into a 3-element array.
pub fn parse_tuple3(value: &str) -> Option<[f64; 3]> {
    let values = parse_tuple(value)?;
    if values.len() != 3 {
        return None;
    }
    Some([values[0], values[1], values[2]])
}

/// A value used as text: surrounding double quotes removed, then trimmed.
pub fn parse_text(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_and_pairs_in_order() {
        let ini = Ini::parse("[A]\nx=1\ny=2\n[B]\nz=3\n");
        assert_eq!(ini.sections.len(), 2);
        assert_eq!(ini.value("a", "x"), Some("1"));
        assert_eq!(ini.value("b", "z"), Some("3"));
    }

    #[test]
    fn keeps_repeated_keys_in_order() {
        let ini = Ini::parse("[L]\nr=(1, 2)\nr=(3, 4)\nr=(5, 6)\n");
        let values: Vec<_> = ini.values("l", "r").collect();
        assert_eq!(values, vec!["(1, 2)", "(3, 4)", "(5, 6)"]);
    }

    #[test]
    fn strips_comments_and_blanks() {
        let ini = Ini::parse("[A] // header\nx = 1 // one\n\n// whole line\n");
        assert_eq!(ini.value("a", "x"), Some("1"));
    }

    #[test]
    fn keys_and_sections_are_case_insensitive() {
        let ini = Ini::parse("[General]\nMASS=750\n");
        assert_eq!(ini.value("GENERAL", "mass"), Some("750"));
    }

    #[test]
    fn entries_before_any_header_land_in_an_unnamed_section() {
        let ini = Ini::parse("RPMTorque=(0, -1, -2)\nRPMTorque=(500, 3, 4)\n");
        assert_eq!(ini.section("").map(|s| s.entries.len()), Some(2));
    }

    #[test]
    fn ignores_lines_without_equals() {
        let ini = Ini::parse("[SLIPCURVE]\nName=\"X\"\nData:\n0.0 0.1 0.2\n");
        assert_eq!(
            ini.value("slipcurve", "name").map(parse_text),
            Some("X".into())
        );
    }

    #[test]
    fn text_strips_quotes() {
        assert_eq!(parse_text("\"Comet\""), "Comet");
        assert_eq!(parse_text("REAR"), "REAR");
        assert_eq!(parse_text(" \"A\" "), "A");
    }

    #[test]
    fn tuple_parsing_allows_spaces_and_exponents() {
        assert_eq!(
            parse_tuple3("( 0.0, -20.0, -20.0)"),
            Some([0.0, -20.0, -20.0])
        );
        assert_eq!(
            parse_tuple("(1.43e-002,6.64e-004)"),
            Some(vec![1.43e-2, 6.64e-4])
        );
        assert_eq!(parse_tuple3("(1)"), None);
    }

    #[test]
    fn parse_usize_accepts_whole_numbers() {
        assert_eq!(parse_usize("4"), Some(4));
        assert_eq!(parse_usize("4.0"), Some(4));
        assert_eq!(parse_usize("0"), Some(0));
        assert_eq!(parse_usize(" 7 "), Some(7));
    }

    #[test]
    fn parse_usize_rejects_bad_numbers() {
        assert_eq!(parse_usize("-1"), None);
        assert_eq!(parse_usize("1.5"), None);
        assert_eq!(parse_usize("nan"), None);
        assert_eq!(parse_usize("NaN"), None);
        assert_eq!(parse_usize("inf"), None);
        assert_eq!(parse_usize("-inf"), None);
        assert_eq!(parse_usize("1e30"), None);
        assert_eq!(parse_usize(""), None);
        assert_eq!(parse_usize("abc"), None);
    }

    #[test]
    fn parse_usize_rejects_values_beyond_usize_range() {
        // 2^64, the first whole f64 above `usize::MAX` on a 64-bit target.
        assert_eq!(parse_usize("18446744073709551616"), None);
        assert_eq!(parse_usize("18446744073709551616.0"), None);
        // A large value safely inside the range still parses.
        assert_eq!(parse_usize("1048576"), Some(1_048_576));
    }

    #[test]
    fn ranged_resolves_min_plus_step_times_setting() {
        let ini = Ini::parse("[CONTROLS]\nSteerLockRange=(5.0, 0.5, 37)\nSteerLockSetting=30\n");
        let ranged = ini
            .section("CONTROLS")
            .unwrap()
            .ranged("SteerLock")
            .unwrap();
        assert_eq!(ranged.value, 20.0);
        assert_eq!(ranged.count, 37.0);
    }

    #[test]
    fn ranged_is_none_when_missing() {
        let section = Section::default();
        assert!(section.ranged("RevLimit").is_none());
    }
}
