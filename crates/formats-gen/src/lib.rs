//! Parser for the gMotor `.veh` / `.gen` vehicle graphics configuration
//! (behaviour described in the format specifications).
//!
//! Both files are plain text; the caller decodes the bytes (Latin-1) and hands
//! the parser a `&str`. Keys are case-insensitive and a trailing `//` comment
//! is stripped from every line.

/// Where a `MASFile` entry is resolved: the team folder or the vehicles root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenDir {
    /// The folder that holds the team's `.veh` files.
    Team,
    /// The nearest ancestor folder named `Vehicles`.
    Vehicles,
}

/// One `MASFile=` line together with the `SearchPath=` it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntry {
    pub dir: GenDir,
    pub mas: String,
}

/// The graphics-file name from a `.veh` file (`Graphics=` line).
///
/// The value may carry trailing spaces and a `//` comment; both are removed.
/// Returns `None` when no non-empty `Graphics=` line is present.
pub fn veh_graphics(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(value) = value_of(line, "graphics") {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// Every `MASFile=` entry of a `.gen` file in file order.
///
/// Each entry is paired with the most recent `SearchPath=` line. An unknown
/// `SearchPath=` value clears the current directory, so the `MASFile=` lines
/// that follow are skipped until the next known `SearchPath=`. Only plain
/// `SearchPath=`/`MASFile=` lines count: a line prefixed with a token such as
/// `<LOW>` is ignored.
pub fn search_path(text: &str) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    let mut current: Option<GenDir> = None;
    for line in text.lines() {
        if let Some(value) = value_of(line, "searchpath") {
            current = match value.to_ascii_uppercase().as_str() {
                "<TEAMDIR>" => Some(GenDir::Team),
                "<VEHDIR>" => Some(GenDir::Vehicles),
                _ => None,
            };
        } else if let Some(value) = value_of(line, "masfile") {
            if let (Some(dir), false) = (current, value.is_empty()) {
                entries.push(SearchEntry { dir, mas: value });
            }
        }
    }
    entries
}

/// Value of the `GenString=` line of a `.veh` file.
///
/// Same rules as [`veh_graphics`]: trailing spaces and a `//` comment are
/// removed, the key is case-insensitive. Returns `None` when absent or empty.
pub fn veh_gen_string(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(value) = value_of(line, "genstring") {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// One mesh of a `.gen` file: the enclosing instance and the expanded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenMesh {
    pub instance: String,
    pub mesh: String,
}

/// Replace `<...>` name tokens using the `.veh` `GenString` value.
///
/// `<digits>` becomes the concatenation of the GenString characters at those
/// 1-based positions (a position past the end contributes nothing), `<ID>`
/// becomes `000`, and any other token is kept verbatim.
pub fn expand_tokens(name: &str, gen_string: &str) -> String {
    let chars: Vec<char> = gen_string.chars().collect();
    let mut out = String::new();
    let mut rest = name;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('>') {
            Some(end) => {
                let token = &after[..end];
                if token.eq_ignore_ascii_case("id") {
                    out.push_str("000");
                } else if !token.is_empty() && token.chars().all(|c| c.is_ascii_digit()) {
                    for digit in token.chars().filter_map(|c| c.to_digit(10)) {
                        if digit >= 1 {
                            if let Some(ch) = chars.get(digit as usize - 1) {
                                out.push(*ch);
                            }
                        }
                    }
                } else {
                    out.push('<');
                    out.push_str(token);
                    out.push('>');
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Tags kept by the viewer's "MAX detail, race view" rule.
const KEPT_TAGS: [&str; 2] = ["MAX", "NOTSPIN"];

/// The mesh lines of a `.gen` file to show, in file order.
///
/// A line is kept when every tag on it is `MAX` or `NOTSPIN` (no tag = keep),
/// `Render` is not `False`, `ShadowObject` is not `True`, and `LODIn` is `0`.
/// Exact duplicate names inside the same instance are dropped. Instance and
/// mesh names are token-expanded (see [`expand_tokens`]) and upper-cased.
pub fn car_meshes(gen_text: &str, gen_string: &str) -> Vec<GenMesh> {
    let mut out: Vec<GenMesh> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut pending: Option<String> = None;
    // Instances with `AnimFile=` need animation to be placed; skip them.
    let mut animated: Vec<String> = Vec::new();

    for raw in gen_text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let (tags, content) = split_tags(line);

        if let Some(name) = assignment(content, "instance") {
            pending = Some(expand_tokens(&name, gen_string).to_ascii_uppercase());
            continue;
        }
        if content == "{" {
            if let Some(name) = pending.take() {
                stack.push(name);
            }
            continue;
        }
        if content == "}" {
            stack.pop();
            continue;
        }

        if assignment(content, "animfile").is_some() {
            if let Some(instance) = stack.last() {
                animated.push(instance.clone());
            }
            continue;
        }

        let Some((mesh_name, keep)) = parse_mesh_line(content) else {
            continue;
        };
        if !keep || !tags.iter().all(|tag| KEPT_TAGS.contains(&tag.as_str())) {
            continue;
        }
        let Some(instance) = stack.last() else {
            continue;
        };
        let mesh = expand_tokens(&mesh_name, gen_string).to_ascii_uppercase();
        if out
            .iter()
            .any(|existing| existing.instance == *instance && existing.mesh == mesh)
        {
            continue;
        }
        out.push(GenMesh {
            instance: instance.clone(),
            mesh,
        });
    }

    out.retain(|mesh| !animated.contains(&mesh.instance));
    out
}

/// Split leading `<TAG>` groups from the rest of a line.
fn split_tags(line: &str) -> (Vec<String>, &str) {
    let mut tags = Vec::new();
    let mut rest = line.trim_start();
    while let Some(inner) = rest.trim_start().strip_prefix('<') {
        let Some(end) = inner.find('>') else {
            break;
        };
        tags.push(inner[..end].to_ascii_uppercase());
        rest = &inner[end + 1..];
    }
    (tags, rest.trim())
}

/// Parse one `MeshFile=NAME key=value ...` content line.
///
/// Returns the mesh name and whether the viewer rule keeps it (ignoring tags).
/// `None` when the content is not a `MeshFile=` line.
fn parse_mesh_line(content: &str) -> Option<(String, bool)> {
    let mut tokens = content.split_whitespace();
    let first = tokens.next()?;
    let (key, mesh) = first.split_once('=')?;
    if !key.eq_ignore_ascii_case("meshfile") || mesh.is_empty() {
        return None;
    }

    let mut render = true;
    let mut shadow = false;
    let mut lod_in = 0.0f32;
    for token in tokens {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.eq_ignore_ascii_case("render") {
            render = !value.eq_ignore_ascii_case("false");
        } else if key.eq_ignore_ascii_case("shadowobject") {
            shadow = value.eq_ignore_ascii_case("true");
        } else if key.eq_ignore_ascii_case("lodin") {
            lod_in = parse_paren_f32(value).unwrap_or(0.0);
        }
    }

    Some((mesh.to_string(), render && !shadow && lod_in == 0.0))
}

/// Parse a `(1.25)`-style value.
fn parse_paren_f32(value: &str) -> Option<f32> {
    value
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .parse()
        .ok()
}

/// Value of a plain `key=value` content line.
fn assignment(content: &str, key: &str) -> Option<String> {
    let (found_key, value) = content.split_once('=')?;
    if found_key.trim().eq_ignore_ascii_case(key) {
        Some(value.trim().to_string())
    } else {
        None
    }
}

/// Value of a plain `key=value` line, comment stripped and trimmed.
///
/// Returns `None` when the line is not a plain assignment to `key` (for
/// example when it starts with a token like `<LOW>` before `key=`).
fn value_of(line: &str, key: &str) -> Option<String> {
    let (found_key, value) = strip_comment(line).split_once('=')?;
    if !found_key.trim().eq_ignore_ascii_case(key) {
        return None;
    }
    Some(value.trim().to_string())
}

/// Text before the first `//`, or the whole line when there is none.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(index) => &line[..index],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veh_graphics_reads_value_ignoring_comment_and_spaces() {
        let text = "Number=27\nGraphics=1994_Generic_F1.gen   // the gen\n";
        assert_eq!(veh_graphics(text).as_deref(), Some("1994_Generic_F1.gen"));
    }

    #[test]
    fn veh_graphics_is_case_insensitive() {
        assert_eq!(
            veh_graphics("graphics=BAR.gen\n").as_deref(),
            Some("BAR.gen")
        );
    }

    #[test]
    fn veh_graphics_ignores_commented_lines() {
        let text = "//graphics=NO.gen\nNumber=27\n";
        assert!(veh_graphics(text).is_none());
    }

    #[test]
    fn veh_graphics_missing_is_none() {
        assert!(veh_graphics("Number=27\nTEAM=\"Ferrari\"\n").is_none());
    }

    #[test]
    fn search_path_pairs_mas_with_search_path() {
        let text = "\
SearchPath=<TEAMDIR>
MASFile=Team.mas
SearchPath=<VEHDIR>
MASFile=1994.mas
MASFile=Cdb.mas
MASFile=Cmaps.mas
MASFile=Drivers.mas
";
        let expected = vec![
            SearchEntry {
                dir: GenDir::Team,
                mas: "Team.mas".to_string(),
            },
            SearchEntry {
                dir: GenDir::Vehicles,
                mas: "1994.mas".to_string(),
            },
            SearchEntry {
                dir: GenDir::Vehicles,
                mas: "Cdb.mas".to_string(),
            },
            SearchEntry {
                dir: GenDir::Vehicles,
                mas: "Cmaps.mas".to_string(),
            },
            SearchEntry {
                dir: GenDir::Vehicles,
                mas: "Drivers.mas".to_string(),
            },
        ];
        assert_eq!(search_path(text), expected);
    }

    #[test]
    fn search_path_strips_comments_and_spaces() {
        let text = "SearchPath=<VEHDIR> // root\nMASFile=1994.mas   // season\n";
        assert_eq!(
            search_path(text),
            vec![SearchEntry {
                dir: GenDir::Vehicles,
                mas: "1994.mas".to_string(),
            }]
        );
    }

    #[test]
    fn search_path_skips_token_prefixed_lines() {
        let text = "SearchPath=<VEHDIR>\n<LOW>MASFile=low.mas\nMASFile=main.mas\n";
        assert_eq!(
            search_path(text),
            vec![SearchEntry {
                dir: GenDir::Vehicles,
                mas: "main.mas".to_string(),
            }]
        );
    }

    #[test]
    fn search_path_unknown_dir_skips_until_known() {
        let text = "\
SearchPath=<UNKNOWN>
MASFile=skip1.mas
MASFile=skip2.mas
SearchPath=<TEAMDIR>
MASFile=Team.mas
";
        assert_eq!(
            search_path(text),
            vec![SearchEntry {
                dir: GenDir::Team,
                mas: "Team.mas".to_string(),
            }]
        );
    }

    #[test]
    fn search_path_keys_are_case_insensitive() {
        let text = "searchpath=<teamdir>\nmasfile=team.mas\n";
        assert_eq!(
            search_path(text),
            vec![SearchEntry {
                dir: GenDir::Team,
                mas: "team.mas".to_string(),
            }]
        );
    }

    #[test]
    fn search_path_without_any_search_path_ignores_masfiles() {
        assert!(search_path("MASFile=orphan.mas\n").is_empty());
    }

    // --- veh_gen_string ---

    #[test]
    fn veh_gen_string_reads_value_ignoring_comment() {
        let text = "GenString=GB28FERBV                 // Used to generate\n";
        assert_eq!(veh_gen_string(text).as_deref(), Some("GB28FERBV"));
    }

    #[test]
    fn veh_gen_string_is_case_insensitive_and_missing_is_none() {
        assert_eq!(veh_gen_string("genstring=abc\n").as_deref(), Some("abc"));
        assert!(veh_gen_string("Graphics=x.gen\n").is_none());
    }

    // --- expand_tokens ---

    #[test]
    fn expand_tokens_positions_concatenate() {
        let gen = "GB28FERBV";
        assert_eq!(expand_tokens("<1234>va.MTS", gen), "GB28va.MTS");
        assert_eq!(expand_tokens("<567>bb.MTS", gen), "FERbb.MTS");
        assert_eq!(expand_tokens("<34>helma.MTS", gen), "28helma.MTS");
        assert_eq!(expand_tokens("<8>DriverArms.MTS", gen), "BDriverArms.MTS");
        assert_eq!(expand_tokens("<1>tlfa.MTS", gen), "Gtlfa.MTS");
    }

    #[test]
    fn expand_tokens_id_and_past_end_and_unknown() {
        let gen = "GB28FERBV";
        assert_eq!(expand_tokens("<ID>", gen), "000");
        assert_eq!(expand_tokens("<9>x", gen), "Vx");
        assert_eq!(expand_tokens("<12>x", gen), "GBx");
        assert_eq!(expand_tokens("<0>x", gen), "x");
        assert_eq!(expand_tokens("<VEHDIR>x", gen), "<VEHDIR>x");
        assert_eq!(expand_tokens("plain.MTS", gen), "plain.MTS");
        assert_eq!(expand_tokens("<99>x", ""), "x");
    }

    // --- car_meshes ---

    #[test]
    fn car_meshes_keeps_max_and_untagged_and_rejects_low_spin() {
        let text = "\
Instance=SLOT<ID>
{
<NOTSPIN>MeshFile=<567>car.MTS Render=False LODIn=(0.0)
<SPIN>   MeshFile=<1234>va.MTS LODIn=(0.0)
<MAX>    MeshFile=<1234>va.MTS LODIn=(0.0) LODOut=(7.0)
<LOW>    MeshFile=<567>bb.MTS LODIn=(0.0)
<MAX>    MeshFile=<567>bb.MTS LODIn=(0.0)
}
";
        let meshes = car_meshes(text, "GB28FERBV");
        assert_eq!(
            meshes,
            vec![
                GenMesh {
                    instance: "SLOT000".to_string(),
                    mesh: "GB28VA.MTS".to_string(),
                },
                GenMesh {
                    instance: "SLOT000".to_string(),
                    mesh: "FERBB.MTS".to_string(),
                },
            ]
        );
    }

    #[test]
    fn car_meshes_drops_shadow_and_nonzero_lodin_and_dedupes() {
        let text = "\
Instance=BODY
{
<MAX> MeshFile=<567>Driver.MTS LODIn=(0.0)
<MAX> MeshFile=<567>Driver.MTS LODIn=(0.0)
<MAX> MeshFile=<567>Driver.MTS ShadowObject=True LODIn=(0.0)
<MAX> MeshFile=<567>Driver.MTS LODIn=(1.0) LODOut=(9.0)
}
";
        let meshes = car_meshes(text, "GB28FERBV");
        assert_eq!(
            meshes,
            vec![GenMesh {
                instance: "BODY".to_string(),
                mesh: "FERDRIVER.MTS".to_string(),
            }]
        );
    }

    #[test]
    fn car_meshes_handles_nesting_and_comments() {
        let text = "\
Instance=DUMMY1
{
  Moveable=True
<NOTSPIN><DASHHIGH> MeshFile=<567>holder.MTS LODIn=(0.0)
  Instance=ARMS
  {
<MAX> MeshFile=<8>DriverArms.MTS LODIn=(0.0) // arms
  }
}
";
        let meshes = car_meshes(text, "GB28FERBV");
        assert_eq!(
            meshes,
            vec![GenMesh {
                instance: "ARMS".to_string(),
                mesh: "BDRIVERARMS.MTS".to_string(),
            }]
        );
    }

    #[test]
    fn car_meshes_skips_animated_instances() {
        let text = "Instance=ARMS
{
<MAX> MeshFile=<8>DriverArms.MTS LODIn=(0.0)
   AnimFile=catsteer.ANM
}
Instance=BODY
{
MeshFile=<567>Driver.MTS LODIn=(0.0)
}
";
        let meshes = car_meshes(text, "GB28FERBV");
        assert_eq!(
            meshes,
            vec![GenMesh {
                instance: "BODY".to_string(),
                mesh: "FERDRIVER.MTS".to_string(),
            }]
        );
    }

    #[test]
    fn car_meshes_keeps_untagged_lines_with_no_tags() {
        let text = "Instance=RAINLIGHT\n{\nMeshFile=<567>rnlt.MTS LODIn=(0.0)\n}\n";
        let meshes = car_meshes(text, "GB28FERBV");
        assert_eq!(
            meshes,
            vec![GenMesh {
                instance: "RAINLIGHT".to_string(),
                mesh: "FERRNLT.MTS".to_string(),
            }]
        );
    }
}
