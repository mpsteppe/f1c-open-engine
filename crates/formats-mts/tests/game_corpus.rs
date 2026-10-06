//! Corpus test: parse every `*.MTS` entry in every `*.mas` under
//! `$F1C_GAME_DIR`.
//!
//! The game directory is never committed. When `F1C_GAME_DIR` is unset the
//! test prints `skipped` and passes.

use std::path::Path;

use formats_mas::MasArchive;
use formats_mts::{parse, MtsError};

#[derive(Default)]
struct Stats {
    ok: usize,
    unsupported: usize,
    errors: usize,
    triangles: usize,
    error_samples: Vec<String>,
}

#[test]
fn parses_every_mts_in_game_dir() {
    let dir = match std::env::var("F1C_GAME_DIR") {
        Ok(dir) => dir,
        Err(_) => {
            println!("skipped");
            return;
        }
    };

    let mut stats = Stats::default();
    walk(Path::new(&dir), &mut stats);

    println!(
        "mts_ok={} unsupported={} triangles={}",
        stats.ok, stats.unsupported, stats.triangles
    );
    assert_eq!(
        stats.errors, 0,
        "{} MTS entries failed to parse; first samples: {:?}",
        stats.errors, stats.error_samples
    );
}

fn walk(dir: &Path, stats: &mut Stats) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for item in read_dir.flatten() {
        let path = item.path();
        if path.is_dir() {
            walk(&path, stats);
            continue;
        }
        if !has_extension(&path, "mas") {
            continue;
        }

        let archive =
            MasArchive::open(&path).unwrap_or_else(|err| panic!("open {}: {err}", path.display()));
        for entry in archive.entries() {
            if !has_name_extension(&entry.name, "mts") {
                continue;
            }
            let bytes = archive
                .read(entry)
                .unwrap_or_else(|err| panic!("read {} in {}: {err}", entry.name, path.display()));
            match parse(&bytes) {
                Ok(mts) => {
                    stats.ok += 1;
                    stats.triangles += mts
                        .groups
                        .iter()
                        .map(|group| group.triangles.len())
                        .sum::<usize>();
                }
                Err(MtsError::Unsupported { .. }) => stats.unsupported += 1,
                Err(err) => {
                    stats.errors += 1;
                    if stats.error_samples.len() < 5 {
                        stats.error_samples.push(format!(
                            "{} :: {} :: {err}",
                            path.display(),
                            entry.name
                        ));
                    }
                }
            }
        }
    }
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
}

fn has_name_extension(name: &str, ext: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
}
