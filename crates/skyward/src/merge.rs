//! A merge followed on a thread of its own, for the screen: the merge
//! itself, and everything it tells the bootloader, is
//! [`skyward_vab::merge::follow`].

use anyhow::Result;
use skyward_sys::worker::Worker;
use skyward_vab::{Merge, Slot};
use std::os::fd::RawFd;

pub enum Event {
    /// How a look at the merge went.
    Looked(Merge),
    /// The merge could not be started or followed, and why.
    Failed(String),
}

/// A merge followed on a thread of its own.
pub struct Merger {
    /// The screen lets it go once the merge has ended.
    worker: Worker<Event>,
}

impl Merger {
    /// Follow the merge into `slot` as [`skyward_vab::merge::follow`] does,
    /// starting it first where `start`. The slot's snapshots have to be
    /// mapped already, as [`skyward_vab::map_snapshots`] maps them.
    pub fn start(slot: Slot, start: bool) -> Result<Merger> {
        let worker = Worker::start("merge", move |tell| {
            if let Err(e) = skyward_vab::merge::follow(slot, start, &mut |merge| {
                tell.tell(Event::Looked(*merge));
            }) {
                tell.tell(Event::Failed(format!("{e:#}")));
            }
        })?;
        Ok(Merger { worker })
    }

    /// Readable while events wait.
    pub fn fd(&self) -> RawFd {
        self.worker.fd()
    }

    /// The events since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.worker.events()
    }

    /// Whether the merge is no longer followed: it ended, or failed.
    pub fn ended(&self) -> bool {
        self.worker.ended()
    }
}
