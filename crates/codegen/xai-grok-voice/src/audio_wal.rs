//! Append-only audio write-ahead log (sibling of prompt WAL).
//!
//! Grok OSS: Until the Operator stops recording, audio bytes are forked with
//! near-zerocopy (append-only CoW-style stream): one path writes through an fd
//! to disk (audio WAL, sibling of prompt WAL under session dir), the other
//! goes to the STT API. Pick the cheaper fork (less memory). Stopping
//! recording finishes the WAL. A later crash/rebuild can still have the bytes
//! on disk.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Leaf name next to `prompt_wal.jsonl` under the session directory.
pub const AUDIO_WAL_FILE: &str = "audio_wal.pcm";

/// Path for this session directory's audio WAL.
pub fn audio_wal_path(session_dir: impl AsRef<Path>) -> PathBuf {
    session_dir.as_ref().join(AUDIO_WAL_FILE)
}

/// Open file descriptor that appends PCM chunks as they arrive.
///
/// Does not keep a growing `Vec` of the whole recording. Each `append` writes
/// the just-captured buffer and returns that same slice for the STT sink.
pub struct AudioWal {
    file: File,
    path: PathBuf,
    finished: bool,
}

impl AudioWal {
    /// Create or truncate the WAL at `path` and keep the fd open for appends.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        Ok(Self {
            file,
            path,
            finished: false,
        })
    }

    /// Path this WAL writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether `finish` has run.
    pub fn is_open(&self) -> bool {
        !self.finished
    }

    /// Write this chunk through the fd (no clone of the whole recording).
    ///
    /// Returns the same slice so the caller can feed STT without a second
    /// allocation of the capture buffer. Fsyncs so a crash still has prefix
    /// bytes on disk.
    pub fn append<'a>(&mut self, chunk: &'a [u8]) -> io::Result<&'a [u8]> {
        self.file.write_all(chunk)?;
        self.file.flush()?;
        self.file.sync_all()?;
        Ok(chunk)
    }

    /// Flush, fsync, and close so crash/rebuild still has the bytes.
    pub fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.file.flush()?;
        self.file.sync_all()?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for AudioWal {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.file.flush();
            let _ = self.file.sync_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Fake STT sink that records the same slices the WAL forked.
    struct FakeSttSink {
        bytes: Vec<u8>,
    }

    impl FakeSttSink {
        fn new() -> Self {
            Self { bytes: Vec::new() }
        }
        fn send(&mut self, chunk: &[u8]) {
            self.bytes.extend_from_slice(chunk);
        }
    }

    fn temp_wal_path() -> PathBuf {
        static N: Mutex<u32> = Mutex::new(0);
        let mut n = N.lock().expect("wal test counter");
        *n += 1;
        // nextest runs each test in its own process, so `n` always starts at 1.
        // Include pid so parallel tests do not share one /tmp WAL inode.
        std::env::temp_dir().join(format!(
            "grok-oss-audio-wal-test-{}-{n}.pcm",
            std::process::id()
        ))
    }

    #[test]
    fn audio_wal_forks_chunks_to_disk_and_stt_without_cloning_the_whole_recording() {
        // Until the Operator stops recording, audio bytes are forked with near-zerocopy (append-only CoW-style stream): one path writes through an fd to disk (audio WAL, sibling of prompt WAL under session dir), the other goes to the STT API. Pick the cheaper fork (less memory). Stopping recording finishes the WAL. A later crash/rebuild can still have the bytes on disk.
        let path = temp_wal_path();
        let _ = std::fs::remove_file(&path);
        let mut wal = AudioWal::open(&path).expect("open audio WAL");
        assert!(wal.is_open());
        let mut stt = FakeSttSink::new();
        let chunks: [&[u8]; 3] = [b"pcm-one", b"pcm-two", b"pcm-three"];
        for chunk in chunks {
            let forked = wal.append(chunk).expect("append");
            assert!(
                std::ptr::eq(forked.as_ptr(), chunk.as_ptr()),
                "STT must reuse the just-captured buffer, not a clone of the whole recording"
            );
            stt.send(forked);
        }
        wal.finish().expect("finish WAL");
        assert!(!wal.is_open());
        drop(wal);
        let on_disk = std::fs::read(&path).expect("reload after finish");
        let sent: Vec<u8> = chunks.concat();
        assert_eq!(
            on_disk, sent,
            "finish then crash-by-drop: file equals bytes sent to the fake STT sink"
        );
        assert_eq!(stt.bytes, sent);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn audio_wal_drop_without_finish_still_has_fsynced_prefix() {
        // Until the Operator stops recording, audio bytes are forked with near-zerocopy (append-only CoW-style stream): one path writes through an fd to disk (audio WAL, sibling of prompt WAL under session dir), the other goes to the STT API. Pick the cheaper fork (less memory). Stopping recording finishes the WAL. A later crash/rebuild can still have the bytes on disk.
        let path = temp_wal_path();
        let _ = std::fs::remove_file(&path);
        let mut wal = AudioWal::open(&path).expect("open");
        let chunk = b"prefix-pcm";
        wal.append(chunk).expect("append fsyncs");
        drop(wal);
        let on_disk = std::fs::read(&path).expect("prefix after drop");
        assert_eq!(
            on_disk, chunk,
            "drop without finish still has the fsynced prefix on disk"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn audio_wal_lives_beside_prompt_wal_filename() {
        let session_dir = PathBuf::from("/tmp/sessions/abc-123");
        let path = audio_wal_path(&session_dir);
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some(AUDIO_WAL_FILE)
        );
        assert_eq!(path.parent().map(Path::to_path_buf), Some(session_dir));
        assert_ne!(AUDIO_WAL_FILE, "prompt_wal.jsonl");
    }
}
