//! The memory controller's rate, held for a panel driven faster than the
//! recovery's own bandwidth vote covers.
//!
//! A display controller brought up by the recovery has no vendor services
//! behind it, so it asks the DVFS service for no bandwidth of its own: the
//! kernel log shows its vote as `ddr 0 MB/s` at the lowest clock profile.
//! At 90 Hz that is enough. Above it the panel reads the frame buffer
//! faster than the memory delivers it, the controller underflows, and the
//! picture tears or the backlight drops. The memory controller's own
//! devfreq node still works, so its fastest rate is held as the floor while
//! such a panel is driven.

use anyhow::{Context, Result};
use std::fs;

/// Where the vendor driver exposes the memory controller's rate.
const DDR: &str = "/sys/class/devfreq/ddr_devfreq";

/// Hold the memory controller at its fastest rate, so a panel scanned out
/// above 90 Hz is fed. Failing leaves the panel driven at whatever rate the
/// controller manages, so callers tell the log and carry on.
pub fn hold_memory_fast() -> Result<()> {
    let rates = fs::read_to_string(format!("{DDR}/available_frequencies"))
        .with_context(|| format!("read {DDR}/available_frequencies"))?;
    let fastest = rates
        .split_whitespace()
        .filter_map(|rate| rate.parse::<u64>().ok())
        .max()
        .with_context(|| format!("{DDR}/available_frequencies names no rate"))?;
    fs::write(format!("{DDR}/min_freq"), fastest.to_string())
        .with_context(|| format!("hold {DDR} at {fastest} Hz"))?;
    Ok(())
}
