//! Parser for the gMotor `.SCN` scene description (track circuits).
//!
//! The file is plain text (Latin-1); the caller decodes the bytes and hands the
//! parser a `&str`. Rules match the `.gen` parser: `//` starts a comment (whole
//! line or trailing), keys are case-insensitive, and `key=value` pairs inside a
//! block may share one whitespace-separated line. Braces nest and are matched
//! by depth, whatever opened the block (`Instance=`, `View=`, `Light=`, ...).
//!
//! Parenthesised values may contain spaces (`Color=(255, 252, 213)`); those
//! spaces are ignored while tokenising, so a line can still be split on
//! whitespace.

/// One parsed `.SCN` file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    /// `SearchPath=` values, in file order (folder names relative to game root).
    pub search_paths: Vec<String>,
    /// `MASFile=` values, in file order.
    pub mas_files: Vec<String>,
    /// Every `Instance=` block, outer and nested, in file order.
    pub instances: Vec<SceneInstance>,
    /// `AmbientColor=` (0..255 per channel).
    pub ambient_color: Option<[u8; 3]>,
    /// `FogMode=...` line, when present.
    pub fog: Option<Fog>,
    /// `View=mainview` `Color=` = background clear colour.
    pub view_color: Option<[u8; 3]>,
    /// `View=mainview` `ClipPlanes=` far value = draw distance (metres).
    pub clip_far: Option<f32>,
    /// Every `Light=` block, in file order.
    pub lights: Vec<SceneLight>,
    /// Non-fatal metadata diagnostics, for example an invalid `HATTarget=`
    /// value. Never silently converted to `true`.
    pub diagnostics: Vec<String>,
}

/// A boolean scene flag kept distinct as absent, true, false or invalid.
///
/// Keys and `True`/`False` values are case-insensitive; an unrecognised value
/// keeps its raw text in [`FlagValue::Invalid`] and produces a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagValue {
    /// The key was not present for this mesh occurrence.
    Absent,
    /// The key was present with a `True` value.
    True,
    /// The key was present with a `False` value.
    False,
    /// The key was present with an unrecognised value.
    Invalid(String),
}

impl FlagValue {
    /// Whether the flag is an explicit `True`.
    pub fn is_true(&self) -> bool {
        matches!(self, FlagValue::True)
    }
}

/// One `MeshFile=` occurrence and the flags that apply to it.
///
/// Flags attach to the latest `MeshFile=` in the same instance, including keys
/// on later lines; a new `MeshFile=` starts fresh flag values, and flags never
/// leak across instances. Duplicate keys keep the last value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshRef {
    /// The `MeshFile=` value as written.
    pub name: String,
    /// `HATTarget=` state for this occurrence.
    pub hat_target: FlagValue,
    /// `CollTarget=` state for this occurrence.
    pub coll_target: FlagValue,
}

impl MeshRef {
    fn new(name: String) -> Self {
        Self {
            name,
            hat_target: FlagValue::Absent,
            coll_target: FlagValue::Absent,
        }
    }
}

/// One `FogMode=` line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fog {
    /// Mode as written, e.g. `LINEAR`.
    pub mode: String,
    /// `FogIn=` start distance (metres).
    pub in_distance: f32,
    /// `FogOut=` end distance (metres).
    pub out_distance: f32,
    /// `FogDensity=` value.
    pub density: f32,
    /// `FogColor=` (0..255 per channel).
    pub color: Option<[u8; 3]>,
}

/// One `Light=` block.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneLight {
    /// The name after `Light=`, e.g. `FDirect01`.
    pub name: String,
    /// `Type=` as written, e.g. `Directional`, `StaticOmni`.
    pub light_type: String,
    /// `Dir=` (game axes) for directional lights.
    pub dir: Option<[f32; 3]>,
    /// `Color=` (0..255 per channel).
    pub color: Option<[u8; 3]>,
}

/// One `Instance=` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneInstance {
    pub name: String,
    /// Index of the enclosing `Instance=` block, if this one is nested.
    pub parent: Option<usize>,
    /// Every `MeshFile=` value inside the block, in file order.
    pub meshes: Vec<String>,
    /// Parallel to `meshes`: the flags of each mesh occurrence, in file order.
    pub mesh_refs: Vec<MeshRef>,
    /// False when the block carries `Render=False`.
    pub render: bool,
    /// True when the block carries `Moveable=True`.
    pub moveable: bool,
    /// True when the block carries a non-empty `AnimFile=`.
    pub animated: bool,
}

impl SceneInstance {
    fn new(name: String) -> Self {
        Self {
            name,
            parent: None,
            meshes: Vec::new(),
            mesh_refs: Vec::new(),
            render: true,
            moveable: false,
            animated: false,
        }
    }
}

/// The block a `{` opened, so keys can be routed to the right owner.
enum Frame {
    /// The instance being parsed and the mesh occurrence its flags attach to.
    Instance {
        index: usize,
        current_mesh: Option<usize>,
    },
    View(String),
    Light(usize),
    Other,
}

/// Parse plain-text `.SCN` content into a [`Scene`].
pub fn parse(text: &str) -> Scene {
    let mut scene = Scene::default();
    let mut instances: Vec<SceneInstance> = Vec::new();
    let mut lights: Vec<SceneLight> = Vec::new();
    // One entry per open block.
    let mut stack: Vec<Frame> = Vec::new();
    // The block named by the most recent `Instance=`/`View=`/`Light=` line,
    // waiting for its `{`.
    let mut pending: Option<Frame> = None;

    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let collapsed = collapse_parentheses(line);
        for token in collapsed.split_whitespace() {
            if token == "{" {
                stack.push(pending.take().unwrap_or(Frame::Other));
                continue;
            }
            if token == "}" {
                stack.pop();
                continue;
            }
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            if key.eq_ignore_ascii_case("instance") {
                if !value.is_empty() {
                    let parent = enclosing_instance(&stack);
                    let index = instances.len();
                    let mut instance = SceneInstance::new(value.to_string());
                    instance.parent = parent;
                    instances.push(instance);
                    pending = Some(Frame::Instance {
                        index,
                        current_mesh: None,
                    });
                }
            } else if key.eq_ignore_ascii_case("view") {
                pending = Some(Frame::View(value.to_string()));
            } else if key.eq_ignore_ascii_case("light") {
                let index = lights.len();
                lights.push(SceneLight {
                    name: value.to_string(),
                    ..Default::default()
                });
                pending = Some(Frame::Light(index));
            } else if key.eq_ignore_ascii_case("searchpath") {
                if !value.is_empty() {
                    scene.search_paths.push(value.to_string());
                }
            } else if key.eq_ignore_ascii_case("masfile") {
                if !value.is_empty() {
                    scene.mas_files.push(value.to_string());
                }
            } else if key.eq_ignore_ascii_case("ambientcolor") {
                if let Some(color) = parse_color(value) {
                    scene.ambient_color = Some(color);
                }
            } else if key.eq_ignore_ascii_case("fogmode") {
                scene.fog.get_or_insert_with(Fog::default).mode = value.to_string();
            } else if key.eq_ignore_ascii_case("fogin") {
                if let Some(distance) = parse_scalar(value) {
                    scene.fog.get_or_insert_with(Fog::default).in_distance = distance;
                }
            } else if key.eq_ignore_ascii_case("fogout") {
                if let Some(distance) = parse_scalar(value) {
                    scene.fog.get_or_insert_with(Fog::default).out_distance = distance;
                }
            } else if key.eq_ignore_ascii_case("fogdensity") {
                if let Some(density) = parse_scalar(value) {
                    scene.fog.get_or_insert_with(Fog::default).density = density;
                }
            } else if key.eq_ignore_ascii_case("fogcolor") {
                if let Some(color) = parse_color(value) {
                    scene.fog.get_or_insert_with(Fog::default).color = Some(color);
                }
            } else if let Some(frame) = stack.last_mut() {
                match frame {
                    Frame::Instance {
                        index,
                        current_mesh,
                    } => {
                        let instance = &mut instances[*index];
                        if key.eq_ignore_ascii_case("render") {
                            instance.render = !value.eq_ignore_ascii_case("false");
                        } else if key.eq_ignore_ascii_case("moveable") {
                            instance.moveable = value.eq_ignore_ascii_case("true");
                        } else if key.eq_ignore_ascii_case("animfile") {
                            if !value.is_empty() {
                                instance.animated = true;
                            }
                        } else if key.eq_ignore_ascii_case("meshfile") && !value.is_empty() {
                            instance.meshes.push(value.to_string());
                            instance.mesh_refs.push(MeshRef::new(value.to_string()));
                            *current_mesh = Some(instance.mesh_refs.len() - 1);
                        } else if key.eq_ignore_ascii_case("hattarget") {
                            apply_flag(
                                instance,
                                current_mesh,
                                value,
                                |mesh| &mut mesh.hat_target,
                                "HATTarget",
                                &mut scene.diagnostics,
                            );
                        } else if key.eq_ignore_ascii_case("colltarget") {
                            apply_flag(
                                instance,
                                current_mesh,
                                value,
                                |mesh| &mut mesh.coll_target,
                                "CollTarget",
                                &mut scene.diagnostics,
                            );
                        }
                    }
                    Frame::View(name) if name.eq_ignore_ascii_case("mainview") => {
                        if key.eq_ignore_ascii_case("color") {
                            if let Some(color) = parse_color(value) {
                                scene.view_color = Some(color);
                            }
                        } else if key.eq_ignore_ascii_case("clipplanes") {
                            if let Some(far) = parse_clip_far(value) {
                                scene.clip_far = Some(far);
                            }
                        }
                    }
                    Frame::Light(index) => {
                        let light = &mut lights[*index];
                        if key.eq_ignore_ascii_case("type") {
                            light.light_type = value.to_string();
                        } else if key.eq_ignore_ascii_case("dir") {
                            light.dir = parse_direction(value);
                        } else if key.eq_ignore_ascii_case("color") {
                            light.color = parse_color(value);
                        }
                    }
                    Frame::View(_) | Frame::Other => {}
                }
            }
        }
    }

    scene.instances = instances;
    scene.lights = lights;
    scene
}

/// Index of the innermost open `Instance=` block, if any.
fn enclosing_instance(stack: &[Frame]) -> Option<usize> {
    stack.iter().rev().find_map(|frame| match frame {
        Frame::Instance { index, .. } => Some(*index),
        _ => None,
    })
}

/// Parse a `True`/`False` flag value, keeping any other text as invalid.
fn parse_flag(value: &str) -> FlagValue {
    if value.eq_ignore_ascii_case("true") {
        FlagValue::True
    } else if value.eq_ignore_ascii_case("false") {
        FlagValue::False
    } else {
        FlagValue::Invalid(value.to_string())
    }
}

/// Attach a flag to the latest mesh occurrence of the instance being parsed.
///
/// Flags before any `MeshFile=` are ignored. An invalid value is recorded in
/// `diagnostics` and stored verbatim, never converted to `true`.
fn apply_flag(
    instance: &mut SceneInstance,
    current_mesh: &Option<usize>,
    value: &str,
    field: impl Fn(&mut MeshRef) -> &mut FlagValue,
    key: &str,
    diagnostics: &mut Vec<String>,
) {
    let Some(index) = *current_mesh else {
        return;
    };
    let parsed = parse_flag(value);
    if let FlagValue::Invalid(raw) = &parsed {
        diagnostics.push(format!(
            "{} {}: invalid {key}={raw}",
            instance.name, instance.mesh_refs[index].name
        ));
    }
    *field(&mut instance.mesh_refs[index]) = parsed;
}

/// Drop whitespace inside `(...)` groups so a line stays whitespace-splittable.
fn collapse_parentheses(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut depth = 0u32;
    for ch in line.chars() {
        match ch {
            '(' => {
                depth += 1;
                out.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                out.push(ch);
            }
            c if c.is_whitespace() && depth > 0 => {}
            c => out.push(c),
        }
    }
    out
}

/// Three `0..255` channels from `(r, g, b)`.
fn parse_color(value: &str) -> Option<[u8; 3]> {
    let inner = strip_parens(value);
    let mut parts = inner.split(',').map(str::trim);
    let r = parts.next()?.parse().ok()?;
    let g = parts.next()?.parse().ok()?;
    let b = parts.next()?.parse().ok()?;
    Some([r, g, b])
}

/// Three components from `(x, y, z)`.
fn parse_direction(value: &str) -> Option<[f32; 3]> {
    let inner = strip_parens(value);
    let mut parts = inner.split(',').map(str::trim);
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    Some([x, y, z])
}

/// One number, optionally wrapped in parentheses (`(100.00)`).
fn parse_scalar(value: &str) -> Option<f32> {
    strip_parens(value).parse().ok()
}

/// The second value of `(near, far)`.
fn parse_clip_far(value: &str) -> Option<f32> {
    let inner = strip_parens(value);
    let mut parts = inner.split(',').map(str::trim);
    let _near: f32 = parts.next()?.parse().ok()?;
    let far: f32 = parts.next()?.parse().ok()?;
    Some(far)
}

/// Remove one layer of surrounding parentheses, then trim.
fn strip_parens(value: &str) -> &str {
    value
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
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
    fn parses_search_path_and_mas_files() {
        let text = "\
SearchPath=SeasonData\\Circuits\\Australia\\1994_Adelaide
SearchPath=SeasonData\\Vehicles
MASFile=Adelaide.mas
MASFile=Cdb.mas   // season
";
        let scene = parse(text);
        assert_eq!(
            scene.search_paths,
            vec![
                "SeasonData\\Circuits\\Australia\\1994_Adelaide".to_string(),
                "SeasonData\\Vehicles".to_string(),
            ]
        );
        assert_eq!(
            scene.mas_files,
            vec!["Adelaide.mas".to_string(), "Cdb.mas".to_string()]
        );
    }

    #[test]
    fn parses_one_instance_with_mesh_and_flags() {
        let text = "\
Instance=TRACK02
{
  MeshFile=TRACK02.mts CollTarget=True HATTarget=True ShadowReceiver=True
}
";
        let scene = parse(text);
        assert_eq!(scene.instances.len(), 1);
        let instance = &scene.instances[0];
        assert_eq!(instance.name, "TRACK02");
        assert_eq!(instance.parent, None);
        assert_eq!(instance.meshes, vec!["TRACK02.mts".to_string()]);
        assert!(instance.render);
        assert!(!instance.moveable);
        assert!(!instance.animated);
    }

    #[test]
    fn render_false_hides_and_moveable_marks() {
        let text = "\
Instance=Xpitout
{
  Render=False
  MeshFile=Xpitout.mts CollTarget=True HATTarget=False
}
Instance=Helicopter
{
  Moveable=True Pos=(0.2,0.1,0.2) Orient=(0.0,0.0,0.0) Planes=(4)
  MeshFile=Helicopter.MTS CollTarget=False HATTarget=False
  AnimFile=HELI_ADEL.ANM AutoStart=True
}
";
        let scene = parse(text);
        assert!(!scene.instances[0].render);
        assert_eq!(scene.instances[0].meshes.len(), 1);
        assert!(scene.instances[1].moveable);
        assert!(scene.instances[1].animated);
    }

    #[test]
    fn nested_instances_are_collected_and_braces_balance() {
        let text = "\
Instance=skyboxi
{
  Moveable=True Change=False Planes=(4)
  MeshFile=skyboxi.MTS CollTarget=False HATTarget=False
  Instance=clouds
  {
    Moveable=True Planes=(4)
    MeshFile=clouds.MTS CollTarget=False HATTarget=False
  }
}
";
        let scene = parse(text);
        assert_eq!(scene.instances.len(), 2);
        assert_eq!(scene.instances[0].name, "skyboxi");
        assert_eq!(scene.instances[0].meshes, vec!["skyboxi.MTS".to_string()]);
        assert_eq!(scene.instances[0].parent, None);
        assert_eq!(scene.instances[1].name, "clouds");
        assert_eq!(scene.instances[1].parent, Some(0));
        assert_eq!(scene.instances[1].meshes, vec!["clouds.MTS".to_string()]);
        assert!(scene.instances[1].moveable);
    }

    #[test]
    fn view_and_light_blocks_do_not_create_instances() {
        let text = "\
View=mainview
{
  Clear=False
  Color=(255, 252, 213)
  View=rearview
  {
    Color=(1, 2, 3)
    ClipPlanes=(1.00, 150.00)
  }
}
Light=FDirect01
{
  Type=Directional Dir=(0.13, -0.82, -0.56) Color=(255, 255, 255)
}
Instance=TRACK02
{
  MeshFile=TRACK02.mts
}
";
        let scene = parse(text);
        assert_eq!(scene.instances.len(), 1);
        assert_eq!(scene.instances[0].name, "TRACK02");
        assert_eq!(scene.lights.len(), 1);
    }

    #[test]
    fn multiple_meshes_per_instance_and_comments_and_case() {
        let text = "\
//Instance=COMMENTED
INSTANCE=WALLS
{
  MESHFILE=A.MTS Render=True
  // MeshFile=HIDDEN.MTS
  meshfile=B.MTS CollTarget=True // keep
}
";
        let scene = parse(text);
        assert_eq!(scene.instances.len(), 1);
        assert_eq!(
            scene.instances[0].meshes,
            vec!["A.MTS".to_string(), "B.MTS".to_string()]
        );
        assert!(scene.instances[0].render);
    }

    #[test]
    fn parses_mainview_color_and_far_clip_ignoring_rearview() {
        let text = "\
View=mainview
{
  Clear=False
  Color=(255, 252, 213)
  Size=(1.0, 1.0) Center=(0.5, 0.5)
  ClipPlanes=(1.00, 1400.00)
  View=rearview
  {
    Color=(10, 20, 30)
    ClipPlanes=(1.00, 150.00)
  }
}
";
        let scene = parse(text);
        assert_eq!(scene.view_color, Some([255, 252, 213]));
        assert_eq!(scene.clip_far, Some(1400.0));
    }

    #[test]
    fn parses_ambient_and_multi_key_fog_line() {
        let text = "\
AmbientColor=(138, 138, 140)
FogMode=LINEAR FogIn=(100.00) FogOut=(3000.00) FogDensity=(0.00) FogColor=(235, 253, 247)
";
        let scene = parse(text);
        assert_eq!(scene.ambient_color, Some([138, 138, 140]));
        let fog = scene.fog.unwrap();
        assert_eq!(fog.mode, "LINEAR");
        assert_eq!(fog.in_distance, 100.0);
        assert_eq!(fog.out_distance, 3000.0);
        assert_eq!(fog.density, 0.0);
        assert_eq!(fog.color, Some([235, 253, 247]));
    }

    #[test]
    fn parses_first_light_blocks() {
        let text = "\
Light=FDirect01
{
 Type=Directional Dir=(0.13, -0.82, -0.56) Color=(255, 255, 255)
}
Light=Omni01
{
 Type=StaticOmni Pos=(206.295, 4.902, -185.409) Range=(0.000, 3.000) Intensity=(-0.300) Color=(255, 255, 255)
}
";
        let scene = parse(text);
        assert_eq!(scene.lights.len(), 2);
        let sun = &scene.lights[0];
        assert_eq!(sun.name, "FDirect01");
        assert_eq!(sun.light_type, "Directional");
        assert_eq!(sun.dir, Some([0.13, -0.82, -0.56]));
        assert_eq!(sun.color, Some([255, 255, 255]));
        assert_eq!(scene.lights[1].light_type, "StaticOmni");
        assert_eq!(scene.lights[1].dir, None);
    }

    #[test]
    fn empty_is_empty() {
        assert_eq!(parse("CUBEASF\n\n"), Scene::default());
    }

    // --- per-mesh HATTarget / CollTarget metadata ---

    fn flags(scene: &Scene, instance: usize, mesh: usize) -> (&FlagValue, &FlagValue) {
        let reference = &scene.instances[instance].mesh_refs[mesh];
        (&reference.hat_target, &reference.coll_target)
    }

    #[test]
    fn flags_bind_to_the_latest_mesh_including_later_lines() {
        let text = "\
Instance=WALLS
{
  MeshFile=A.MTS
  HATTarget=True
  CollTarget=True
  MeshFile=B.MTS
  HATTarget=False
}
";
        let scene = parse(text);
        assert_eq!(scene.instances[0].meshes, vec!["A.MTS", "B.MTS"]);
        assert_eq!(flags(&scene, 0, 0), (&FlagValue::True, &FlagValue::True));
        assert_eq!(flags(&scene, 0, 1), (&FlagValue::False, &FlagValue::Absent));
    }

    #[test]
    fn same_line_flags_bind_to_that_mesh() {
        let text = "\
Instance=TRACK02
{
  MeshFile=TRACK02.mts CollTarget=True HATTarget=True ShadowReceiver=True
}
";
        let scene = parse(text);
        assert_eq!(flags(&scene, 0, 0), (&FlagValue::True, &FlagValue::True));
    }

    #[test]
    fn absent_true_false_and_invalid_are_distinct_and_case_insensitive() {
        let text = "\
Instance=A
{
  MeshFile=A.MTS hattarget=TRUE colltarget=maybe
  MeshFile=B.MTS HATtarget=FALSE
  MeshFile=C.MTS
}
";
        let scene = parse(text);
        assert_eq!(
            flags(&scene, 0, 0),
            (&FlagValue::True, &FlagValue::Invalid("maybe".to_string()))
        );
        assert_eq!(flags(&scene, 0, 1), (&FlagValue::False, &FlagValue::Absent));
        assert_eq!(
            flags(&scene, 0, 2),
            (&FlagValue::Absent, &FlagValue::Absent)
        );
        assert_eq!(scene.diagnostics.len(), 1);
        assert!(
            scene.diagnostics[0].contains("CollTarget=maybe"),
            "{:?}",
            scene.diagnostics
        );
    }

    #[test]
    fn duplicate_flag_keeps_the_last_value() {
        let text = "\
Instance=A
{
  MeshFile=A.MTS
  HATTarget=True
  HATTarget=False
  HATTarget=True
}
";
        let scene = parse(text);
        assert_eq!(flags(&scene, 0, 0).0, &FlagValue::True);
    }

    #[test]
    fn flags_do_not_leak_across_instances_or_nested_blocks() {
        let text = "\
Instance=OUTER
{
  MeshFile=OUTER.MTS HATTarget=True
  Instance=INNER
  {
    MeshFile=INNER.MTS
  }
}
Instance=SIBLING
{
  MeshFile=SIBLING.MTS
}
";
        let scene = parse(text);
        assert_eq!(flags(&scene, 0, 0).0, &FlagValue::True);
        assert_eq!(flags(&scene, 1, 0).0, &FlagValue::Absent);
        assert_eq!(flags(&scene, 2, 0).0, &FlagValue::Absent);
    }

    #[test]
    fn render_false_still_carries_flags_and_colltarget_only_is_recorded() {
        let text = "\
Instance=HIDDEN
{
  Render=False
  MeshFile=HIDDEN.MTS HATTarget=True
}
Instance=COLL
{
  MeshFile=COLL.MTS CollTarget=True
}
";
        let scene = parse(text);
        assert!(!scene.instances[0].render);
        assert_eq!(flags(&scene, 0, 0).0, &FlagValue::True);
        assert_eq!(flags(&scene, 1, 0), (&FlagValue::Absent, &FlagValue::True));
    }

    #[test]
    fn flag_without_a_mesh_is_ignored() {
        let text = "\
Instance=NOMESH
{
  HATTarget=True
}
";
        let scene = parse(text);
        assert!(scene.instances[0].mesh_refs.is_empty());
        assert!(scene.diagnostics.is_empty());
    }

    #[test]
    fn comments_follow_existing_scn_rules() {
        let text = "\
Instance=A
{
  MeshFile=A.MTS HATTarget=True // comment
  // HATTarget=False
  CollTarget=True
}
";
        let scene = parse(text);
        assert_eq!(flags(&scene, 0, 0), (&FlagValue::True, &FlagValue::True));
    }
}
