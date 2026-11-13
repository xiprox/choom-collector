//! SHA-256 of the files in mod folders, worked out again only when a file's size or modified time
//! changes, a few megabytes a tick so a large archive never holds up the session's thread.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

/// A file's SHA-256, which is also how the hub's cache names it.
pub type Hash = [u8; 32];

/// The SHA-256 of no bytes, which a follower writes without fetching.
pub const EMPTY: Hash = [
    0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95,
    0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
];

pub fn hex(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// When a file was last seen as it was hashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub size: u64,
    pub modified: Option<SystemTime>,
}

impl Stamp {
    pub fn of(path: &Path) -> Option<Stamp> {
        let m = fs::metadata(path).ok()?;
        m.is_file().then(|| Stamp { size: m.len(), modified: m.modified().ok() })
    }
}

struct Job {
    path: PathBuf,
    stamp: Stamp,
    sha: Sha256,
    done: u64,
    urgent: bool,
}

#[derive(Default)]
pub struct Hashes {
    known: HashMap<PathBuf, (Stamp, Hash)>,
    urgent: VecDeque<PathBuf>,
    idle: VecDeque<PathBuf>,
    queued: HashSet<PathBuf>,
    job: Option<Job>,
    /// An idle job set aside for an urgent one, picked up where it stopped.
    paused: Option<Job>,
}

impl Hashes {
    /// The file's hash when it's known for this size and time; otherwise the file is queued, and
    /// a later call has it. `urgent` puts the sharer's and the followed files ahead of the
    /// comparisons the window shows.
    pub fn get(&mut self, path: &Path, stamp: Stamp, urgent: bool) -> Option<Hash> {
        if let Some((s, h)) = self.known.get(path)
            && *s == stamp
        {
            return Some(*h);
        }
        let working = [&self.job, &self.paused].into_iter().flatten().any(|j| j.path == path);
        if !working && self.queued.insert(path.to_path_buf()) {
            let q = if urgent { &mut self.urgent } else { &mut self.idle };
            q.push_back(path.to_path_buf());
        }
        None
    }

    /// Records a hash the caller knows, such as a file the follower just put in place.
    pub fn set(&mut self, path: &Path, stamp: Stamp, hash: Hash) {
        self.known.insert(path.to_path_buf(), (stamp, hash));
    }

    #[cfg(test)]
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.paused.is_some() || !self.urgent.is_empty() || !self.idle.is_empty()
    }

    /// Reads up to `budget` bytes of the queued files.
    pub fn work(&mut self, budget: u64) {
        let mut left = budget;
        let mut buf = vec![0u8; 256 * 1024];
        while left > 0 {
            if self.job.as_ref().is_some_and(|j| !j.urgent) && !self.urgent.is_empty() {
                self.paused = self.job.take();
            }
            if self.job.is_none() {
                self.job = self.next_job();
            }
            let Some(job) = self.job.as_mut() else { return };
            let read = (|| -> std::io::Result<usize> {
                let mut f = File::open(&job.path)?;
                f.seek(SeekFrom::Start(job.done))?;
                let mut got = 0;
                while left > 0 && job.done < job.stamp.size {
                    let want = (buf.len() as u64).min(left).min(job.stamp.size - job.done) as usize;
                    let n = f.read(&mut buf[..want])?;
                    if n == 0 {
                        break;
                    }
                    job.sha.update(&buf[..n]);
                    job.done += n as u64;
                    left -= n as u64;
                    got += n;
                }
                Ok(got)
            })();
            let finished = match read {
                Ok(n) => job.done >= job.stamp.size || n == 0,
                Err(_) => true,
            };
            if !finished {
                continue;
            }
            let job = self.job.take().expect("a job was just worked on");
            // A file that changed while it was read gets queued again by the next caller to ask.
            if job.done == job.stamp.size && Stamp::of(&job.path) == Some(job.stamp) {
                self.known.insert(job.path, (job.stamp, job.sha.finalize().into()));
            }
        }
    }

    fn next_job(&mut self) -> Option<Job> {
        loop {
            let (path, urgent) = if let Some(p) = self.urgent.pop_front() {
                (p, true)
            } else if let Some(j) = self.paused.take() {
                return Some(j);
            } else if let Some(p) = self.idle.pop_front() {
                (p, false)
            } else {
                return None;
            };
            self.queued.remove(&path);
            // The file may have changed since it was queued, and what's there now is what's asked
            // for next.
            if let Some(stamp) = Stamp::of(&path) {
                return Some(Job { path, stamp, sha: Sha256::new(), done: 0, urgent });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_file_is_hashed_across_several_calls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.archive");
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(&path, &data).unwrap();
        let stamp = Stamp::of(&path).unwrap();
        let mut h = Hashes::default();
        assert_eq!(h.get(&path, stamp, true), None);
        h.work(1_000_000);
        assert!(h.busy());
        assert_eq!(h.get(&path, stamp, true), None);
        h.work(1_000_000);
        h.work(1_000_000);
        assert!(!h.busy());
        let want: Hash = Sha256::digest(&data).into();
        assert_eq!(h.get(&path, stamp, true), Some(want));
    }

    #[test]
    fn the_empty_hash_is_the_hash_of_nothing() {
        let want: Hash = Sha256::digest([]).into();
        assert_eq!(EMPTY, want);
    }
}
