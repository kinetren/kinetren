//! Disk calls a gesture asks for, off the UI thread (found-defects 840, the class of 911 and 1002): the job runs
//! on a lane of its own, and what it answers lands within the frame when the disk is quick, in a later
//! frame when it is not. A window that asked the disk in its frame stood with it.

use crate::app::{KtApp, media_grace};
use std::time::Duration;

type Land = Box<dyn FnOnce(&mut KtApp) + Send>;

/// An answer on its way: `Some` — it came; `None` with the flag — it never will.
pub(crate) struct DiskAsk(Box<dyn FnMut(Duration) -> Option<Land> + Send>, Box<dyn Fn() -> bool + Send>);

/// One lane, in order: a folder made and then opened, a file removed and written again.
pub(crate) fn lane() -> &'static kt_time::sched::LaneRef {
    static L: std::sync::OnceLock<kt_time::sched::LaneRef> = std::sync::OnceLock::new();
    L.get_or_init(|| kt_time::sched::Lane::open("ui-disk", kt_time::sched::Class::Answer))
}

impl KtApp {
    /// Run `job` on the disk lane; `land` gets its answer on the UI thread.
    pub(crate) fn disk<T: Send + 'static>(
        &mut self,
        job: impl FnOnce() -> T + Send + 'static,
        land: impl FnOnce(&mut KtApp, T) + Send + 'static,
    ) {
        let (answer, reply) = kt_time::sched::Answer::new();
        lane().post(move |_| {
            reply.send(job());
            crate::wake::now();
        });
        let answer = std::sync::Arc::new(answer);
        let (gone, mut land) = (answer.clone(), Some(land));
        self.disk_asks.push(DiskAsk(
            Box::new(move |grace| {
                let t = answer.take_within(grace)?;
                let land = land.take()?;
                Some(Box::new(move |app: &mut KtApp| land(app, t)) as Land)
            }),
            Box::new(move || gone.abandoned()),
        ));
        self.take_disk(media_grace());
    }

    /// The answers that came within `grace`, landed.
    pub(crate) fn take_disk(&mut self, grace: Duration) {
        if self.disk_asks.is_empty() {
            return;
        }
        let mut left = Vec::new();
        for mut ask in std::mem::take(&mut self.disk_asks) {
            match (ask.0)(grace) {
                Some(land) => land(self),
                None if (ask.1)() => {}
                None => left.push(ask),
            }
        }
        left.append(&mut self.disk_asks);
        self.disk_asks = left;
    }
}
