use std::cell::UnsafeCell;
use std::mem::replace;
use std::ptr::null;
use std::sync::{Arc, Weak};
use std::thread::{current, park_timeout, Thread};
use std::time::{Duration, Instant};

use scopeguard::guard;
use smallvec::SmallVec;

use crate::stdio::StdinSignal;

#[derive(Default)]
pub(crate) struct PollController {
    min_instant: Option<Instant>,
    //min_systime: Option<SystemTime>,
    signals: SmallVec<[WaitData; 8]>,
}

pub(crate) struct WaitData {
    signal: Weak<StdinSignal>,
    thread: Thread,
    state: UnsafeCell<WaitState>,
    pub(crate) next: UnsafeCell<*const WaitData>,
    pub(crate) prev: UnsafeCell<*const WaitData>,
}

#[derive(PartialEq, Eq)]
enum WaitState {
    Inactive,
    Waiting,
    Waited,
}

impl WaitData {
    #[inline]
    pub(crate) unsafe fn waited(&self) {
        *self.state.get() = WaitState::Waited;
        self.thread.unpark();
    }
}

impl PollController {
    pub(crate) fn set_instant(&mut self, t: Instant) {
        let Some(v) = &mut self.min_instant else {
            self.min_instant = Some(t);
            return;
        };
        *v = (*v).min(t);
    }

    //pub(crate) fn set_systime(&mut self, t: SystemTime) {
    //    let Some(v) = &mut self.min_systime else {
    //        self.min_systime = Some(t);
    //        return;
    //    };
    //    *v = (*v).min(t);
    //}

    pub(crate) fn add_signal(&mut self, signal: &Arc<StdinSignal>) {
        let w = Arc::downgrade(signal);
        if self.signals.iter().all(|v| !Weak::ptr_eq(&v.signal, &w)) {
            self.signals.push(WaitData {
                signal: w,
                thread: current(),
                state: UnsafeCell::new(WaitState::Inactive),
                next: UnsafeCell::new(null()),
                prev: UnsafeCell::new(null()),
            });
        }
    }

    pub(crate) fn poll(&mut self) {
        let mut dur = None;

        if let Some(t) = &self.min_instant {
            let d = t
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::ZERO);
            dur = match dur {
                None => Some(d),
                Some(b) => Some(d.min(b)),
            };
        }

        //if let Some(t) = &self.min_systime {
        //    let d = t
        //        .duration_since(SystemTime::now())
        //        .unwrap_or(Duration::ZERO);
        //    dur = match dur {
        //        None => Some(d),
        //        Some(b) => Some(d.min(b)),
        //    };
        //}

        if dur.map_or(false, |d| d.is_zero()) {
            return;
        }

        let signals = guard(&self.signals, |s| {
            for i in s {
                let Some(s) = i.signal.upgrade() else {
                    continue;
                };
                let mut g = s.inner.lock();

                // SAFETY: Signal is locked, so all nodes are held.
                unsafe {
                    if *i.state.get() != WaitState::Waiting {
                        continue;
                    }

                    let p: *const WaitData = replace(&mut (*i.prev.get()), null());
                    let n: *const WaitData = replace(&mut (*i.next.get()), null());

                    *if p.is_null() {
                        &mut g.head
                    } else {
                        &mut (*(*p).next.get())
                    } = n;

                    if !n.is_null() {
                        *(*n).prev.get() = p;
                    }
                }
            }
        });

        for i in signals.iter() {
            let Some(s) = i.signal.upgrade() else {
                continue;
            };
            let mut g = s.inner.lock();
            if g.is_ready() {
                return;
            }

            // SAFETY: Signal is locked, so all nodes are held.
            unsafe {
                *i.state.get() = WaitState::Waiting;

                if !g.head.is_null() {
                    *i.next.get() = g.head;
                    *(*g.head).prev.get() = i;
                }
            }
            g.head = i;
        }

        // Limit waiting time
        park_timeout(
            dur.unwrap_or_else(|| Duration::from_secs(1))
                .min(Duration::from_secs(5)),
        );
    }
}
