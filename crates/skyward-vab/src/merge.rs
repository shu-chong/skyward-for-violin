//! Following an update's merge to its end, as libsnapshot does in the
//! system it merges into: started where it has not begun, then looked at
//! every two seconds until it ends, the bootloader told where the update
//! stands through the phone's own boot control.
//!
//! The slot merged into is not always the slot the phone runs from: an
//! update written to the other slot is merged there from the recovery,
//! while the data partition its copy-on-write images lie on is open,
//! since the new system's own first start cannot open them.

use crate::boot::BootControl;
use crate::{check_merge, start_merge, Merge, Slot};
use anyhow::{Context, Result};
use skyward_binder::driver::Binder;
use std::thread;
use std::time::Duration;

/// How often a merge under way is looked at, as libsnapshot looks.
const LOOK_EVERY: Duration = Duration::from_secs(2);

/// Merge the update into `slot`, starting the merge first where `start`,
/// and follow it to its end, `each` told how every look at it went.
/// /metadata is writable while it runs, and read-only again after.
///
/// The slot's snapshots have to be mapped as kernel snapshots first; see
/// [`crate::map_snapshots`].
pub fn follow(slot: Slot, start: bool, each: &mut dyn FnMut(&Merge)) -> Result<Merge> {
    let (binder, hardware) = (Binder::open()?, Binder::hardware()?);
    let control = BootControl::open(&binder, &hardware)?;
    let tell = |status| control.set_merge_status(status);
    crate::metadata_writable(true).context("make /metadata writable to merge the update")?;
    let merged = (|| -> Result<Merge> {
        if start {
            start_merge(slot, &tell)?;
        }
        loop {
            let merge = check_merge(slot, &tell)?;
            each(&merge);
            if !matches!(merge, Merge::Going(_)) {
                return Ok(merge);
            }
            thread::sleep(LOOK_EVERY);
        }
    })();
    let back = crate::metadata_writable(false).context("make /metadata read-only again");
    let merge = merged?;
    back?;
    Ok(merge)
}
