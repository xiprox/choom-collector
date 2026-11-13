//! What the hub and a follower refuse from another PC, and the backups' limits.

mod common;

use std::fs;
use std::path::Path;

use common::Net;
use coop_mods::wire::{self, Block, FileEntry, FromHub, Listed, Share, SharedFolder, ToHub};
use coop_mods::{Backups, Follow, Hash, Target};
use sha2::{Digest, Sha256};

const BOSSUN: usize = 1;

fn sha(data: &[u8]) -> Hash {
    Sha256::digest(data).into()
}

fn entry(path: &str, data: &[u8]) -> FileEntry {
    FileEntry { path: path.into(), size: data.len() as u64, hash: sha(data) }
}

fn folder(path: &str, files: Vec<FileEntry>) -> SharedFolder {
    SharedFolder { path: path.into(), changed: 1, files }
}

/// Every file under `dir` named `name`, wherever it landed.
fn find(dir: &Path, name: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(find(&p, name));
            } else if e.file_name() == name {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn the_hub_takes_only_folders_in_a_mod_place_and_files_inside_them() {
    let mut net = Net::new(&["xip", "bossun"]);
    let share = ToHub::Share(Share {
        never_share: vec![],
        folders: vec![
            folder("r6/scripts/../../bin", vec![]),
            folder("/etc", vec![]),
            folder("C:/Windows", vec![]),
            folder("r6/scripts", vec![]),
            folder("r6/scripts/Good/Deeper", vec![]),
            folder(
                "r6/scripts/Good",
                vec![
                    entry("r6/scripts/Good/../../../evil.dll", b"evil"),
                    entry("r6/scripts/Good/sub/../../Other/evil.dll", b"evil"),
                    entry("/r6/scripts/Good/abs.reds", b"abs"),
                    entry("r6/scripts/Other/x.reds", b"x"),
                    entry("r6/scripts/Good/fine.reds", b"fine"),
                ],
            ),
        ],
    });
    let out = net.hub().on_message("install-bossun", &wire::encode(&share));
    let lists: Vec<Vec<Listed>> = out
        .iter()
        .filter(|(t, _)| *t == Target::Everyone)
        .filter_map(|(_, b)| match wire::decode::<FromHub>(b) {
            Some(FromHub::List(l)) => Some(l),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 1);
    let list = &lists[0];
    assert_eq!(list.iter().map(|l| l.path.as_str()).collect::<Vec<_>>(), ["r6/scripts/Good"]);
    assert_eq!(list[0].files, [entry("r6/scripts/Good/fine.reds", b"fine")]);
    assert_eq!(list[0].sharer, "bossun");
}

#[test]
fn the_hub_takes_a_files_data_only_from_its_sharer() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "r6/scripts/Mod/a.reds", b"real");
    net.pm(BOSSUN).share("mod", true);
    assert!(net.run_until(5.0, |n| n.row(2, "mod").is_some()));
    let forged = ToHub::Data(Block { path: "r6/scripts/Mod/a.reds".into(), hash: sha(b"real"), size: 4, offset: 0, data: b"fake".to_vec() });
    net.hub().on_message("install-mira", &wire::encode(&forged));
    net.pm(2).follow("mod");
    assert!(net.run_until(5.0, |n| n.read(2, "r6/scripts/Mod/a.reds").is_some()));
    assert_eq!(net.read(2, "r6/scripts/Mod/a.reds").unwrap(), b"real");
}

#[test]
fn a_follower_checks_every_path_again_before_writing() {
    let mut net = Net::new(&["xip", "bossun"]);
    // The hub's list is forged, as a hub that skipped its own checks would send it.
    let fine = entry("r6/scripts/Good/fine.reds", b"fine");
    let list = FromHub::List(vec![
        Listed { path: "r6/scripts/../../bin".into(), sharer: "eve".into(), pc: "ee".into(), never_share: vec![], away: false, changed: 1, files: vec![entry("r6/scripts/../../bin/evil.dll", b"evil")] },
        Listed { path: "/etc".into(), sharer: "eve".into(), pc: "ee".into(), never_share: vec![], away: false, changed: 1, files: vec![] },
        Listed {
            path: "r6/scripts/Good".into(),
            sharer: "eve".into(),
            pc: "ee".into(),
            never_share: vec![],
            away: false,
            changed: 1,
            files: vec![entry("r6/scripts/Good/../../../../evil.dll", b"evil"), entry("r6/scripts/Elsewhere/evil.dll", b"evil"), fine.clone()],
        },
    ]);
    net.pm(BOSSUN).on_message(&wire::encode(&list));
    let rows = net.p(BOSSUN).shared();
    assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["good"]);
    net.pm(BOSSUN).follow("good");
    assert_eq!(net.row(BOSSUN, "good").unwrap().follow, Follow::Yes);
    let _ = net.pm(BOSSUN).tick();

    let data = |path: &str, body: &[u8]| wire::encode(&FromHub::Data(Block { path: path.into(), hash: sha(body), size: body.len() as u64, offset: 0, data: body.to_vec() }));
    for bad in ["r6/scripts/Good/../../../../evil.dll", "r6/scripts/Elsewhere/evil.dll", "r6/scripts/../../bin/evil.dll", "/r6/scripts/Good/evil.dll", "bin/x64/evil.dll"] {
        net.pm(BOSSUN).on_message(&data(bad, b"evil"));
    }
    // The right path with the wrong bytes doesn't get in either.
    net.pm(BOSSUN).on_message(&data("r6/scripts/Good/fine.reds", b"nope"));
    net.pm(BOSSUN).on_message(&data("r6/scripts/Good/fine.reds", b"fine"));
    let _ = net.pm(BOSSUN).tick();
    assert!(find(&net.root, "evil.dll").is_empty(), "something was written outside the mod folder");
    assert_eq!(net.read(BOSSUN, "r6/scripts/Good/fine.reds").unwrap(), b"fine");
}

#[test]
fn backups_drop_the_oldest_past_the_cap_and_skip_files_too_large() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("mod-backups");
    for (stamp, size) in [("2000-01-01_00-00-00", 60), ("2000-01-02_00-00-00", 60)] {
        fs::create_dir_all(root.join(stamp).join("r6/scripts/Old")).unwrap();
        fs::write(root.join(stamp).join("r6/scripts/Old/x.reds"), vec![0u8; size]).unwrap();
    }
    let small = dir.path().join("small.reds");
    fs::write(&small, vec![1u8; 50]).unwrap();
    let mut backups = Backups::with_limits(root.clone(), 120, 100);
    assert!(backups.keep(&small, "r6/scripts/New/small.reds"));
    backups.trim();
    let mut left: Vec<String> = fs::read_dir(&root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    left.sort();
    assert_eq!(left.len(), 2, "{left:?}");
    assert_eq!(left[0], "2000-01-02_00-00-00", "the oldest went first");
    assert_eq!(fs::read(root.join(&left[1]).join("r6/scripts/New/small.reds")).unwrap(), vec![1u8; 50]);

    let big = dir.path().join("big.archive");
    fs::write(&big, vec![2u8; 200]).unwrap();
    assert!(backups.keep(&big, "archive/pc/mod/New/big.archive"), "a file too large counts as kept");
    assert!(find(&root, "big.archive").is_empty());
}
