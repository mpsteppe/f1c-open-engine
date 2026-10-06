//! Corpus test: parse every `*.hdv` under `$F1C_GAME_DIR`.
//!
//! The game directory is never committed. When `F1C_GAME_DIR` is unset the
//! test prints `skipped` and passes. When it is set, the root must be readable
//! and must contain at least one `.hdv`; a traversal error or an unreadable
//! root is a failure, not silent zero coverage. A file counts as parsed when it
//! has the `[GENERAL]` and `[DRIVELINE]` section headers this check looks for.
//! It does not verify typed values or that linked files resolve; those are
//! covered by the unit tests and by [`CarPhysics::load`].

use std::io;
use std::path::Path;

use formats_hdv::Hdv;

#[derive(Default)]
struct Stats {
    seen: usize,
    parsed: usize,
    failed: usize,
    samples: Vec<String>,
}

#[test]
fn parses_every_hdv_in_game_dir() {
    let dir = match std::env::var("F1C_GAME_DIR") {
        Ok(dir) => dir,
        Err(_) => {
            println!("skipped");
            return;
        }
    };

    let root = Path::new(&dir);
    let mut stats = Stats::default();
    if let Err(error) = walk(root, &mut stats) {
        panic!("cannot traverse F1C_GAME_DIR {}: {error}", root.display());
    }

    println!(
        "root={} seen={} parsed={} failed={}",
        root.display(),
        stats.seen,
        stats.parsed,
        stats.failed
    );
    assert!(
        stats.seen > 0,
        "no .hdv files found under {}; check F1C_GAME_DIR",
        root.display()
    );
    assert_eq!(
        stats.failed, 0,
        "{} HDV files failed; first samples: {:?}",
        stats.failed, stats.samples
    );
}

fn walk(dir: &Path, stats: &mut Stats) -> io::Result<()> {
    let read_dir = std::fs::read_dir(dir)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", dir.display())))?;

    for item in read_dir {
        let item = item
            .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", dir.display())))?;
        let path = item.path();
        if path.is_dir() {
            walk(&path, stats)?;
            continue;
        }
        if !has_extension(&path, "hdv") {
            continue;
        }

        stats.seen += 1;
        let Some(text) = read_latin1(&path) else {
            record_failure(stats, &path, "cannot read");
            continue;
        };
        let hdv = Hdv::parse(&text);
        if hdv.ini.section("GENERAL").is_none() {
            record_failure(stats, &path, "no [GENERAL] section");
        } else if hdv.ini.section("DRIVELINE").is_none() {
            record_failure(stats, &path, "no [DRIVELINE] section");
        } else {
            stats.parsed += 1;
        }
    }
    Ok(())
}

fn record_failure(stats: &mut Stats, path: &Path, reason: &str) {
    stats.failed += 1;
    if stats.samples.len() < 5 {
        stats
            .samples
            .push(format!("{} :: {reason}", path.display()));
    }
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
}

fn read_latin1(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(bytes.into_iter().map(char::from).collect())
}
