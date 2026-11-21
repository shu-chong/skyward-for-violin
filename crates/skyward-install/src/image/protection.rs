//! Temporarily permit writes to block devices an OTA verifier left
//! read-only. Ordinary files never receive a block ioctl.

use anyhow::{ensure, Context, Result};
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

const BLKROSET: libc::c_ulong = 0x125d;
const BLKROGET: libc::c_ulong = 0x125e;

fn ioctl(file: &File, request: libc::c_ulong, flag: &mut libc::c_int) -> io::Result<()> {
    // libc::Ioctl is c_int on bionic and c_ulong on glibc.
    if unsafe { libc::ioctl(file.as_raw_fd(), request as libc::Ioctl, flag) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn read_only(file: &File) -> io::Result<bool> {
    let mut flag = 0;
    ioctl(file, BLKROGET, &mut flag)?;
    Ok(flag != 0)
}

fn set_read_only(file: &File, on: bool) -> io::Result<()> {
    ioctl(file, BLKROSET, &mut libc::c_int::from(on))
}

/// Clears a block device's read-only flag for as long as it is held, and
/// puts it back on drop. Holds the descriptor only when it had to change
/// anything, so devices that were writable are left completely alone.
pub(super) struct ReadOnly {
    path: PathBuf,
    held: Option<File>,
}

impl ReadOnly {
    pub fn clear(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {} read-only", path.display()))?;
        let block = file.metadata().with_context(|| format!("stat {}", path.display()))?.file_type().is_block_device();
        let mut guard = Self { path: path.to_path_buf(), held: None };
        if block && read_only(&file).with_context(|| format!("read the read-only flag of {}", path.display()))? {
            // Arm the guard before changing anything: even a failed check
            // afterwards must put the original flag back.
            guard.held = Some(file);
            let file = guard.held.as_ref().unwrap();
            set_read_only(file, false).with_context(|| format!("clear the read-only flag of {}", path.display()))?;
            ensure!(!read_only(file)?, "{} stays read-only", path.display());
        }
        Ok(guard)
    }

    pub fn restore(&mut self) -> Result<()> {
        if let Some(file) = &self.held {
            set_read_only(file, true).with_context(|| format!("restore the read-only flag of {}", self.path.display()))?;
            ensure!(read_only(file)?, "{} did not become read-only again", self.path.display());
            // Keep the descriptor on failure so Drop can retry and report.
            self.held = None;
        }
        Ok(())
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        if let Err(e) = self.restore() {
            let _ = skyward_sys::kmsg(&format!("install: {e:#}"));
        }
    }
}
