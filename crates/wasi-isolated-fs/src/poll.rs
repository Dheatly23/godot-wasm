use std::cell::UnsafeCell;
use std::mem::replace;
use std::ptr::null;
use std::sync::{Arc, Weak};
use std::thread::{current, park_timeout, sleep, Thread};
use std::time::{Duration, Instant, SystemTime};

use scopeguard::guard;
use smallvec::SmallVec;

use crate::stdio::StdinSignal;

pub(crate) struct PollController {
    min_instant: Option<Instant>,
    min_systime: Option<SystemTime>,
    signals: SmallVec<[WaitData; 8]>,

    timeout: Option<Instant>,
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
    pub(crate) fn new(timeout: Option<Instant>) -> Self {
        Self {
            min_instant: None,
            min_systime: None,
            signals: SmallVec::new(),

            timeout,
        }
    }

    pub(crate) fn set_instant(&mut self, t: Instant) {
        let Some(v) = &mut self.min_instant else {
            self.min_instant = Some(t);
            return;
        };
        *v = (*v).min(t);
    }

    pub(crate) fn set_systime(&mut self, t: SystemTime) {
        let Some(v) = &mut self.min_systime else {
            self.min_systime = Some(t);
            return;
        };
        *v = (*v).min(t);
    }

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

    pub(crate) fn is_waited(&self, signal: &Arc<StdinSignal>) -> bool {
        let w = Arc::downgrade(signal);
        self.signals
            .iter()
            .any(|v| unsafe { *v.state.get() == WaitState::Waited && Weak::ptr_eq(&v.signal, &w) })
    }

    pub(crate) fn poll(&mut self) -> bool {
        let mut dur = None;

        if let Some(t) = &self.min_instant {
            let d = t.saturating_duration_since(Instant::now());
            dur = match dur {
                None => Some(d),
                Some(b) => Some(d.min(b)),
            };
        }

        if let Some(t) = &self.min_systime {
            let d = t
                .duration_since(SystemTime::now())
                .unwrap_or(Duration::ZERO);
            dur = match dur {
                None => Some(d),
                Some(b) => Some(d.min(b)),
            };
        }

        if dur.map_or(false, |d| d.is_zero()) {
            return false;
        }

        let signals = guard(&self.signals, |s| {
            for i in s {
                let Some(s) = i.signal.upgrade() else {
                    continue;
                };
                let mut g = s.inner.lock();

                // SAFETY: Signal is locked, so all nodes are held.
                unsafe {
                    match *i.state.get() {
                        WaitState::Inactive | WaitState::Waited => continue,
                        WaitState::Waiting => *i.state.get() = WaitState::Inactive,
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

        let mut has_waited = false;
        for i in signals.iter() {
            let Some(s) = i.signal.upgrade() else {
                continue;
            };
            let mut g = s.inner.lock();

            // SAFETY: Signal is locked, so all nodes are held.
            unsafe {
                if g.is_ready() || *i.state.get() == WaitState::Waited {
                    *i.state.get() = WaitState::Waited;
                    has_waited = true;
                    continue;
                }
                *i.state.get() = WaitState::Waiting;

                if !g.head.is_null() {
                    *i.next.get() = g.head;
                    *(*g.head).prev.get() = i;
                }
            }
            g.head = i;
        }
        if has_waited {
            return false;
        }

        // Limit waiting time
        let mut dur = dur.unwrap_or_else(|| Duration::from_secs(1));
        match self
            .timeout
            .map(|t| t.checked_duration_since(Instant::now()))
        {
            Some(Some(t)) => dur = dur.min(t),
            None => (),
            Some(None) => return true,
        }
        if signals.is_empty() {
            sleep(dur);
        } else {
            park_timeout(dur);
        }
        self.timeout.map_or(false, |t| t <= Instant::now())
    }
}
