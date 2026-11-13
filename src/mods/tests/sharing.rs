//! Sharing and following between players in one process, through a hub, with a game folder each.

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

use common::{Net, bytes};
use coop_mods::{Delivery, Doing, Follow, Follower, ModStatus, Place};

const HOST: usize = 0;
const BOSSUN: usize = 1;
const MIRA: usize = 2;

fn without(mut files: BTreeMap<String, Vec<u8>>, names: &[&str]) -> BTreeMap<String, Vec<u8>> {
    files.retain(|k, _| !names.contains(&k.as_str()));
    files
}

/// bossun shares `Tweaks` in two places, and mira follows it until her copy is up to date.
fn shared_and_followed() -> Net {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", bytes(1, 700_000));
    net.write(BOSSUN, "r6/scripts/Tweaks/sub/b.reds", b"b");
    net.write(BOSSUN, "r6/tweaks/Tweaks/c.yaml", b"c: 1");
    net.pm(BOSSUN).share("tweaks", true);
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").is_some()));
    net.pm(MIRA).follow("tweaks");
    assert!(net.run_until(10.0, |n| n.row(MIRA, "tweaks").unwrap().status == ModStatus::UpToDate));
    net
}

#[test]
fn a_mod_is_its_folders_of_one_name_across_places() {
    let mut net = Net::new(&["xip", "bossun"]);
    net.write(BOSSUN, "r6/scripts/EquipmentEx/module.reds", b"module");
    net.write(BOSSUN, "r6/scripts/EquipmentEx/notes.log", b"left out");
    net.write(BOSSUN, "archive/pc/mod/equipmentex/EquipmentEx.archive", bytes(1, 1000));
    net.write(BOSSUN, "archive/pc/mod/#EquipmentEx-patch.archive", b"loose");
    net.write(BOSSUN, "bin/x64/plugins/cyber_engine_tweaks/mods/Other Mod/init.lua", b"return {}");
    net.write(BOSSUN, "bin/x64/plugins/cyber_engine_tweaks/mods/Other Mod/db.sqlite3", b"db");
    net.write(BOSSUN, "r6/scripts/logs/x.reds", b"a folder named logs is left out");
    net.run(10.5);

    let mods = net.p(BOSSUN).my_mods();
    assert_eq!(mods.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(), ["equipmentex", "other mod"]);
    let eq = &mods[0];
    assert_eq!(eq.name, "EquipmentEx");
    assert_eq!(eq.places, [Place::Redscript, Place::Archive]);
    assert_eq!(eq.bytes, 6 + 1000);
    assert_eq!(eq.loose_file.as_deref(), Some("archive/pc/mod/#EquipmentEx-patch.archive"));
    assert!(!eq.shared);
    assert_eq!(mods[1].places, [Place::Cet]);
    assert_eq!(mods[1].bytes, 9);

    net.pm(BOSSUN).share("EquipmentEx", true);
    assert!(net.p(BOSSUN).my_mods()[0].shared);
    assert!(net.run_until(3.0, |n| n.row(HOST, "equipmentex").is_some()));
    let row = net.row(HOST, "equipmentex").unwrap();
    assert_eq!((row.name.as_str(), row.sharer.as_str(), row.mine, row.follow), ("EquipmentEx", "bossun", false, Follow::No));
    assert_eq!(row.status, ModStatus::NotOnYourPc);
    assert!(!row.has_dll);
    assert!(row.changed > 0);
    assert_eq!(net.p(HOST).shared().len(), 1, "two folders of one name make one row");
    let own = net.row(BOSSUN, "equipmentex").unwrap();
    assert!(own.mine);
    assert_eq!(own.status, ModStatus::Sharing);

    net.write(BOSSUN, "archive/pc/mod/equipmentex/plugin.dll", b"code");
    assert!(net.run_until(3.0, |n| n.row(HOST, "equipmentex").unwrap().has_dll));
}

#[test]
fn a_followers_copy_becomes_the_same_as_the_sharers() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", bytes(1, 700_000));
    net.write(BOSSUN, "r6/scripts/Tweaks/sub/b.reds", b"b");
    net.write(BOSSUN, "r6/scripts/Tweaks/empty.txt", b"");
    net.write(BOSSUN, "r6/scripts/Tweaks/bossun.log", b"stays out");
    net.write(BOSSUN, "r6/tweaks/Tweaks/c.yaml", b"c: 1");
    // mira has an old copy, a file the sharer doesn't have, and a log of her own.
    net.write(MIRA, "r6/scripts/Tweaks/a.reds", b"old");
    net.write(MIRA, "r6/scripts/Tweaks/extra.reds", b"mine");
    net.write(MIRA, "r6/scripts/Tweaks/mira.log", b"her log");
    net.pm(BOSSUN).share("tweaks", true);
    assert!(net.run_until(6.0, |n| n.row(MIRA, "tweaks").is_some_and(|r| r.status == ModStatus::OnYourPc { same: false })));

    net.pm(MIRA).follow("tweaks");
    assert_eq!(net.row(MIRA, "tweaks").unwrap().follow, Follow::Yes);
    assert!(net.run_until(10.0, |n| n.row(MIRA, "tweaks").unwrap().status == ModStatus::UpToDate));

    assert_eq!(without(net.files(MIRA, "r6/scripts/Tweaks"), &["mira.log"]), without(net.files(BOSSUN, "r6/scripts/Tweaks"), &["bossun.log"]));
    assert_eq!(net.files(MIRA, "r6/tweaks/Tweaks"), net.files(BOSSUN, "r6/tweaks/Tweaks"));
    assert_eq!(net.read(MIRA, "r6/scripts/Tweaks/mira.log").unwrap(), b"her log");
    assert_eq!(net.read(MIRA, "r6/scripts/Tweaks/bossun.log"), None);
    assert_eq!(net.p(MIRA).doing(), Doing::Nothing);

    // Nothing is kept aside: the follower's copy is simply the sharer's (the user, 2026-11-13).
    assert!(!net.pcs[MIRA].data.join("mod-backups").exists());

    assert!(net.run_until(3.0, |n| n.row(BOSSUN, "tweaks").unwrap().followers == [Follower { name: "mira".into(), delivery: Delivery::UpToDate }]));
}


#[test]
fn a_changed_a_new_and_a_deleted_file_reach_the_follower() {
    let mut net = shared_and_followed();
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", bytes(2, 700_000));
    net.write(BOSSUN, "r6/scripts/Tweaks/new.reds", b"new");
    std::fs::remove_file(net.path(BOSSUN, "r6/scripts/Tweaks/sub/b.reds")).unwrap();
    std::fs::remove_dir(net.path(BOSSUN, "r6/scripts/Tweaks/sub")).unwrap();
    assert!(net.run_until(5.0, |n| n.files(MIRA, "r6/scripts/Tweaks") == n.files(BOSSUN, "r6/scripts/Tweaks")), "the change took over 5s");
    assert!(!net.path(MIRA, "r6/scripts/Tweaks/sub").exists());
}

#[test]
fn the_host_follows_through_its_own_hub() {
    let mut net = Net::new(&["xip", "bossun"]);
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"a");
    net.pm(BOSSUN).share("tweaks", true);
    assert!(net.run_until(5.0, |n| n.row(HOST, "tweaks").is_some()));
    net.pm(HOST).follow("tweaks");
    assert!(net.run_until(5.0, |n| n.read(HOST, "r6/scripts/Tweaks/a.reds").as_deref() == Some(b"a")));
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"a2");
    assert!(net.run_until(5.0, |n| n.read(HOST, "r6/scripts/Tweaks/a.reds").as_deref() == Some(b"a2")));
}

#[test]
fn an_edit_in_a_followed_folder_is_undone() {
    let mut net = shared_and_followed();
    net.write(MIRA, "r6/scripts/Tweaks/a.reds", b"hacked");
    net.write(MIRA, "r6/scripts/Tweaks/added.reds", b"mine");
    net.write(MIRA, "r6/tweaks/Tweaks/c.yaml", b"c: 2");
    assert!(net.run_until(3.0, |n| n.files(MIRA, "r6/scripts/Tweaks") == n.files(BOSSUN, "r6/scripts/Tweaks") && n.files(MIRA, "r6/tweaks/Tweaks") == n.files(BOSSUN, "r6/tweaks/Tweaks")));
}

#[test]
fn a_folder_the_sharer_adds_waits_for_the_follower_to_agree() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"a");
    net.pm(BOSSUN).share("tweaks", true);
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").is_some()));
    net.pm(MIRA).follow("tweaks");
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").unwrap().status == ModStatus::UpToDate));

    net.write(BOSSUN, "archive/pc/mod/Tweaks/t.archive", b"archive");
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").unwrap().follow == Follow::AddedFolder));
    assert_eq!(net.row(MIRA, "tweaks").unwrap().status, ModStatus::AddedFolder);
    net.run(4.0);
    assert!(!net.path(MIRA, "archive/pc/mod/Tweaks").exists(), "nothing arrives before she agrees");

    net.pm(MIRA).take_added_folder("tweaks");
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").unwrap().status == ModStatus::UpToDate));
    assert_eq!(net.row(MIRA, "tweaks").unwrap().follow, Follow::Yes);
    assert_eq!(net.read(MIRA, "archive/pc/mod/Tweaks/t.archive").unwrap(), b"archive");
}

fn followers(net: &Net, i: usize, key: &str) -> Vec<(String, Delivery)> {
    net.row(i, key).map(|r| r.followers.into_iter().map(|f| (f.name, f.delivery)).collect()).unwrap_or_default()
}

fn both(bossun: Delivery, mira: Delivery) -> Vec<(String, Delivery)> {
    vec![("bossun".into(), bossun), ("mira".into(), mira)]
}

#[test]
fn each_followers_delivery_reaches_the_sharer() {
    // The host shares here, so its own player sends through the hub in-process.
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(HOST, "r6/scripts/HostMod/a.reds", b"a");
    net.pm(HOST).share("hostmod", true);
    assert!(net.run_until(5.0, |n| n.row(BOSSUN, "hostmod").is_some() && n.row(MIRA, "hostmod").is_some()));
    net.pm(BOSSUN).follow("hostmod");
    net.pm(MIRA).follow("hostmod");
    assert!(net.run_until(5.0, |n| followers(n, HOST, "hostmod") == both(Delivery::UpToDate, Delivery::UpToDate)));

    // A change that lands while mira's game runs needs a restart of it.
    net.pcs[MIRA].game_running.store(true, Ordering::SeqCst);
    net.run(2.5);
    net.write(HOST, "r6/scripts/HostMod/a.reds", b"a2");
    assert!(net.run_until(5.0, |n| followers(n, HOST, "hostmod") == both(Delivery::UpToDate, Delivery::NeedsRestart)));
    assert_eq!(net.row(MIRA, "hostmod").unwrap().status, ModStatus::NeedsRestart);
    assert_eq!(net.p(MIRA).doing(), Doing::NeedsRestart);
    net.pcs[MIRA].game_running.store(false, Ordering::SeqCst);
    assert!(net.run_until(4.0, |n| followers(n, HOST, "hostmod") == both(Delivery::UpToDate, Delivery::UpToDate)));
    assert_eq!(net.p(MIRA).doing(), Doing::Nothing);

    // bossun's link drops, and comes back.
    net.drop_link(BOSSUN);
    assert!(net.run_until(2.0, |n| followers(n, HOST, "hostmod") == both(Delivery::Paused, Delivery::UpToDate)));
    assert_eq!(net.row(BOSSUN, "hostmod").unwrap().status, ModStatus::Paused);
    net.restore_link(BOSSUN);
    assert!(net.run_until(4.0, |n| followers(n, HOST, "hostmod") == both(Delivery::UpToDate, Delivery::UpToDate)));
}

#[test]
fn a_slow_follower_shows_fetching_n_of_m_files() {
    let mut net = Net::new(&["xip", "bossun"]);
    for i in 0..4 {
        net.write(HOST, &format!("archive/pc/mod/Big/{i}.archive"), bytes(i, 600_000));
    }
    net.pm(HOST).share("big", true);
    assert!(net.run_until(5.0, |n| n.row(BOSSUN, "big").is_some()));
    net.pcs[BOSSUN].cap_in = 1;
    net.pm(BOSSUN).follow("big");
    let mut seen = Vec::new();
    for _ in 0..200 {
        net.step();
        let at_host = followers(&net, HOST, "big");
        if let [(_, d @ Delivery::Fetching { .. })] = at_host.as_slice() {
            if !seen.contains(d) {
                seen.push(d.clone());
            }
            let Delivery::Fetching { done, total } = *d else { unreachable!() };
            assert_eq!(net.p(BOSSUN).doing(), Doing::Fetching { done, total });
            assert_eq!(net.row(BOSSUN, "big").unwrap().status, ModStatus::Fetching { done, total });
        }
        if at_host == [("bossun".to_string(), Delivery::UpToDate)] {
            break;
        }
    }
    assert_eq!(followers(&net, HOST, "big"), [("bossun".to_string(), Delivery::UpToDate)]);
    assert!(seen.contains(&Delivery::Fetching { done: 0, total: 4 }), "saw {seen:?}");
    assert!(seen.iter().any(|d| matches!(d, Delivery::Fetching { done: 1..=3, total: 4 })), "saw {seen:?}");
    assert_eq!(net.files(BOSSUN, "archive/pc/mod/Big"), net.files(HOST, "archive/pc/mod/Big"));
}

#[test]
fn unfollowing_with_delete_removes_the_copy() {
    let mut net = shared_and_followed();
    net.write(MIRA, "r6/scripts/Tweaks/db.sqlite3", b"what the mod saved");
    assert_eq!(net.pm(MIRA).unfollow("tweaks", true), Ok(()));
    assert!(!net.path(MIRA, "r6/scripts/Tweaks").exists());
    assert!(!net.path(MIRA, "r6/tweaks/Tweaks").exists());
    assert!(!net.pcs[MIRA].data.join("mod-backups").exists());
    assert!(net.run_until(6.0, |n| n.row(MIRA, "tweaks").unwrap().status == ModStatus::NotOnYourPc));
    assert_eq!(net.row(MIRA, "tweaks").unwrap().follow, Follow::No);
    assert!(net.run_until(3.0, |n| n.row(BOSSUN, "tweaks").unwrap().followers.is_empty()));
}

#[test]
fn unfollowing_without_delete_keeps_the_copy_as_it_is() {
    let mut net = shared_and_followed();
    assert_eq!(net.pm(MIRA).unfollow("tweaks", false), Ok(()));
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"changed after");
    net.run(4.0);
    assert_eq!(net.read(MIRA, "r6/scripts/Tweaks/a.reds").unwrap(), bytes(1, 700_000));
}

#[test]
fn unfollowing_a_linked_folder_deletes_only_the_link() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"a");
    // mira's folder is a link to her working copy elsewhere.
    let work = net.root.join("work").join("Tweaks");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("draft.reds"), b"draft").unwrap();
    std::fs::create_dir_all(net.path(MIRA, "r6/scripts")).unwrap();
    if !link_folder(&net.path(MIRA, "r6/scripts/Tweaks"), &work) {
        eprintln!("skipped: this PC can't make a link to a folder");
        return;
    }
    net.pm(BOSSUN).share("tweaks", true);
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").is_some()));
    net.pm(MIRA).follow("tweaks");
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").unwrap().status == ModStatus::UpToDate));
    assert_eq!(std::fs::read(work.join("a.reds")).unwrap(), b"a", "following writes through the link");

    assert_eq!(net.pm(MIRA).unfollow("tweaks", true), Ok(()));
    assert!(std::fs::symlink_metadata(net.path(MIRA, "r6/scripts/Tweaks")).is_err(), "the link is gone");
    assert!(work.join("a.reds").exists(), "what the link pointed at stays");
}

#[cfg(windows)]
fn link_folder(link: &std::path::Path, target: &std::path::Path) -> bool {
    std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(link).arg(target).output().is_ok_and(|o| o.status.success())
}

#[cfg(unix)]
fn link_folder(link: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[test]
fn the_sharers_patterns_and_the_defaults_stay_out_on_both_sides() {
    let mut net = Net::with_never_share(&["xip", "bossun", "mira"], &[&[] as &[&str], &["*.psd", "wip/"]]);
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"a");
    net.write(BOSSUN, "r6/scripts/Tweaks/art.psd", b"bossun's art");
    net.write(BOSSUN, "r6/scripts/Tweaks/wip/next.reds", b"unfinished");
    net.write(BOSSUN, "r6/scripts/Tweaks/logs/today.txt", b"log");
    net.write(BOSSUN, "r6/scripts/Tweaks/db.sqlite3", b"bossun's db");
    net.write(MIRA, "r6/scripts/Tweaks/art.psd", b"mira's art");
    net.write(MIRA, "r6/scripts/Tweaks/db.sqlite3", b"mira's db");
    net.write(MIRA, "r6/scripts/Tweaks/logs/old.txt", b"mira's log");
    net.pm(BOSSUN).share("tweaks", true);
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").is_some()));
    net.pm(MIRA).follow("tweaks");
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").unwrap().status == ModStatus::UpToDate));
    let files = net.files(MIRA, "r6/scripts/Tweaks");
    let names: Vec<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(names, ["a.reds", "art.psd", "db.sqlite3", "logs/old.txt"]);
    assert_eq!(files["art.psd"], b"mira's art");
    assert_eq!(files["db.sqlite3"], b"mira's db");
}

#[test]
fn a_restart_keeps_what_is_shared_and_followed() {
    let mut net = shared_and_followed();
    net.restart(BOSSUN);
    net.restart(MIRA);
    assert!(net.pcs[MIRA].data.join("mods.toml").exists());
    assert!(net.p(BOSSUN).my_mods().iter().any(|m| m.key == "tweaks" && m.shared));
    assert!(net.run_until(5.0, |n| n.row(MIRA, "tweaks").is_some_and(|r| r.follow == Follow::Yes && r.status == ModStatus::UpToDate)));
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"after the restart");
    assert!(net.run_until(5.0, |n| n.read(MIRA, "r6/scripts/Tweaks/a.reds").as_deref() == Some(b"after the restart")));
}

#[test]
fn a_follow_belongs_to_the_pc_the_player_agreed_to() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "r6/scripts/Shared/a.reds", b"bossun's");
    net.write(MIRA, "r6/scripts/Shared/a.reds", b"mira's");
    net.pm(BOSSUN).share("shared", true);
    net.run(3.0);
    net.pm(MIRA).share("shared", true);
    net.run(3.0);
    assert_eq!(net.row(HOST, "shared").unwrap().sharer, "bossun", "the first to share keeps it");
    assert_eq!(net.row(MIRA, "shared").unwrap().status, ModStatus::OfferedToo);
    net.pm(HOST).follow("shared");
    assert!(net.run_until(5.0, |n| n.read(HOST, "r6/scripts/Shared/a.reds").as_deref() == Some(b"bossun's")));

    net.pm(BOSSUN).share("shared", false);
    assert!(net.run_until(5.0, |n| n.row(HOST, "shared").is_some_and(|r| r.sharer == "mira")));
    let row = net.row(HOST, "shared").unwrap();
    assert_eq!(row.follow, Follow::No);
    assert_eq!(row.status, ModStatus::FollowedElsewhere { by: "bossun".into() });
    net.run(4.0);
    assert_eq!(net.read(HOST, "r6/scripts/Shared/a.reds").unwrap(), b"bossun's", "nothing changes until the player agrees");

    net.pm(HOST).follow("shared");
    assert!(net.run_until(5.0, |n| n.read(HOST, "r6/scripts/Shared/a.reds").as_deref() == Some(b"mira's")));
}

#[test]
fn a_file_on_its_way_picks_up_where_it_stopped() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    let size = 1_500_000;
    net.write(BOSSUN, "archive/pc/mod/Big/big.archive", bytes(9, size));
    net.pm(BOSSUN).share("big", true);
    assert!(net.run_until(5.0, |n| n.row(MIRA, "big").is_some()));
    // bossun's upload goes one message a step, and drops after two blocks reach the hub.
    net.pcs[BOSSUN].cap_out = 1;
    net.pm(MIRA).follow("big");
    let cache = net.cache.clone();
    let cached = move || std::fs::read_dir(&cache).unwrap().flatten().map(|e| e.metadata().unwrap().len()).max().unwrap_or(0);
    assert!(net.run_until(10.0, |_| cached() >= 2 * 256 * 1024));
    let had = cached();
    assert!(had < size as u64);
    net.drop_link(BOSSUN);
    net.step();
    assert_eq!(net.row(MIRA, "big").unwrap().status, ModStatus::Paused, "the sharer is away");
    net.run(3.0);
    net.pcs[BOSSUN].cap_out = usize::MAX;
    net.restore_link(BOSSUN);
    assert!(net.run_until(10.0, |n| n.row(MIRA, "big").unwrap().status == ModStatus::UpToDate));
    assert_eq!(net.read(MIRA, "archive/pc/mod/Big/big.archive").unwrap(), bytes(9, size));
    assert_eq!(net.pcs[BOSSUN].uploaded, size as u64, "nothing the hub had was sent again");
}

#[test]
fn a_sharer_who_leaves_leaves_the_copies_as_they_are() {
    let mut net = shared_and_followed();
    let before = net.files(MIRA, "r6/scripts/Tweaks");
    net.drop_link(BOSSUN);
    net.run(61.0);
    assert_eq!(net.row(MIRA, "tweaks").unwrap().status, ModStatus::NotShared);
    assert_eq!(net.row(MIRA, "tweaks").unwrap().follow, Follow::Yes);
    assert_eq!(net.files(MIRA, "r6/scripts/Tweaks"), before);
}

#[test]
fn the_hubs_cache_keeps_a_file_10_minutes_and_empties_when_the_hub_stops() {
    let mut net = shared_and_followed();
    let count = |n: &Net| std::fs::read_dir(&n.cache).map(|d| d.count()).unwrap_or(0);
    assert!(count(&net) >= 3);
    net.jump(std::time::Duration::from_secs(8 * 60));
    net.run(61.0);
    assert!(count(&net) >= 3, "kept for 10 minutes after its last use");
    net.jump(std::time::Duration::from_secs(60));
    net.run(61.0);
    assert_eq!(count(&net), 0);
    net.write(BOSSUN, "r6/scripts/Tweaks/a.reds", b"again");
    assert!(net.run_until(5.0, |n| n.read(MIRA, "r6/scripts/Tweaks/a.reds").as_deref() == Some(b"again")));
    assert!(count(&net) >= 1);
    net.hub = None;
    assert!(!net.cache.exists());
}

#[test]
fn a_mod_of_thousands_of_files_travels_in_pieces() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    let dir = net.path(BOSSUN, "archive/pc/mod/Textures/textures/characters/player/body/variants");
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..2500u32 {
        std::fs::write(dir.join(format!("a_texture_file_with_a_long_descriptive_name_{i:04}.xbm")), i.to_le_bytes()).unwrap();
    }
    net.pm(BOSSUN).share("textures", true);
    assert!(net.run_until(10.0, |n| n.row(MIRA, "textures").is_some()));
    net.pm(MIRA).follow("textures");
    assert!(net.run_until(20.0, |n| n.row(MIRA, "textures").unwrap().status == ModStatus::UpToDate));
    assert_eq!(net.files(MIRA, "archive/pc/mod/Textures").len(), 2500);
    assert_eq!(net.files(MIRA, "archive/pc/mod/Textures"), net.files(BOSSUN, "archive/pc/mod/Textures"));
    assert!(net.pieces > 0, "the list and the wants are over 300 KB, so they went in pieces");
}

#[test]
fn a_folder_that_exists_with_other_capitals_is_used_as_it_is() {
    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "r6/scripts/MyMod/a.reds", b"a");
    net.write(MIRA, "r6/scripts/mymod/old.reds", b"old");
    net.pm(BOSSUN).share("mymod", true);
    assert!(net.run_until(5.0, |n| n.row(MIRA, "mymod").is_some()));
    net.pm(MIRA).follow("mymod");
    assert!(net.run_until(5.0, |n| n.row(MIRA, "mymod").unwrap().status == ModStatus::UpToDate));
    let names: Vec<String> = std::fs::read_dir(net.path(MIRA, "r6/scripts")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["mymod"]);
    assert_eq!(net.files(MIRA, "r6/scripts/mymod").keys().collect::<Vec<_>>(), ["a.reds"]);
}

/// The running game holds an archive without letting others delete it, which a handle opened
/// without share-delete access does the same way.
#[cfg(windows)]
#[test]
fn a_file_the_game_holds_is_replaced_once_it_lets_go() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 1;

    let mut net = Net::new(&["xip", "bossun", "mira"]);
    net.write(BOSSUN, "archive/pc/mod/Held/held.archive", b"version 1");
    net.pm(BOSSUN).share("held", true);
    assert!(net.run_until(5.0, |n| n.row(MIRA, "held").is_some()));
    net.pm(MIRA).follow("held");
    assert!(net.run_until(5.0, |n| n.row(MIRA, "held").unwrap().status == ModStatus::UpToDate));

    net.pcs[MIRA].game_running.store(true, Ordering::SeqCst);
    net.run(2.5);
    let held = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(net.path(MIRA, "archive/pc/mod/Held/held.archive")).unwrap();
    net.write(BOSSUN, "archive/pc/mod/Held/held.archive", b"version 2, longer");
    assert!(net.run_until(5.0, |n| followers(n, BOSSUN, "held") == [("mira".to_string(), Delivery::NeedsRestart)]));
    assert_eq!(net.row(MIRA, "held").unwrap().status, ModStatus::NeedsRestart);
    net.run(3.0);
    assert_eq!(net.read(MIRA, "archive/pc/mod/Held/held.archive").unwrap(), b"version 1", "the held file stays until it's let go");
    assert!(net.path(MIRA, "archive/pc/mod/Held/held.archive.coop-tmp").exists());

    drop(held);
    assert!(net.run_until(3.0, |n| n.read(MIRA, "archive/pc/mod/Held/held.archive").as_deref() == Some(b"version 2, longer")));
    assert!(!net.path(MIRA, "archive/pc/mod/Held/held.archive.coop-tmp").exists());
    assert_eq!(net.row(MIRA, "held").unwrap().status, ModStatus::NeedsRestart, "the game loaded the old one");
    net.pcs[MIRA].game_running.store(false, Ordering::SeqCst);
    assert!(net.run_until(4.0, |n| followers(n, BOSSUN, "held") == [("mira".to_string(), Delivery::UpToDate)]));
}
