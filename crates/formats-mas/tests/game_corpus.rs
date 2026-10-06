//! Corpus test: read every `*.mas` under `$F1C_GAME_DIR`.
//!
//! The game directory is never committed. When `F1C_GAME_DIR` is unset the
//! test prints `skipped` and passes.

use std::path::Path;

use formats_mas::MasArchive;

#[test]
fn reads_every_archive_in_game_dir() {
    let dir = match std::env::var("F1C_GAME_DIR") {
        Ok(dir) => dir,
        Err(_) => {
            println!("skipped");
            return;
        }
    };

    let mut archives = 0usize;
    let mut entries = 0usize;
    walk(Path::new(&dir), &mut archives, &mut entries);
    println!("archives={archives} entries={entries}");
}

fn walk(dir: &Path, archives: &mut usize, entries: &mut usize) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for item in read_dir.flatten() {
        let path = item.path();
        if path.is_dir() {
            walk(&path, archives, entries);
            continue;
        }
        if !has_mas_extension(&path) {
            continue;
        }

        let archive =
            MasArchive::open(&path).unwrap_or_else(|err| panic!("open {}: {err}", path.display()));
        for entry in archive.entries() {
            archive
                .read(entry)
                .unwrap_or_else(|err| panic!("read {} in {}: {err}", entry.name, path.display()));
            *entries += 1;
        }
        *archives += 1;
    }
}

fn has_mas_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mas"))
}
