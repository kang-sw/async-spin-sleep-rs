//! # Async-spin-sleep
//!
//! A dedicated timer driver implementation for easy use of high-precision sleep function in
//! numerous async/await context.
//!
//! ## Features
//!
//! - *`system-clock`* (default): Enable use of system clock as a timer source.
//!
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicIsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use crossbeam::channel;
use driver::{NodeDesc, WakerCell};

/* -------------------------------------------- Init -------------------------------------------- */
#[cfg(windows)]
const DEFAULT_SCHEDULE_RESOLUTION: Duration = Duration::from_millis(33);
#[cfg(unix)]
const DEFAULT_SCHEDULE_RESOLUTION: Duration = Duration::from_millis(3);

#[derive(Debug, derive_setters::Setters)]
#[setters(prefix = "with_")]
pub struct Builder {
    /// Default scheduling resolution for this driver. Setting this to a lower value may decrease
    /// CPU usage of the driver, but may also dangerously increase the chance of missing a wakeup
    /// event due to the OS scheduler.
    pub schedule_resolution: Duration,

    /// Aborted nodes that are too far from execution may remain in the driver's memory for a long
    /// time. This value specifies the maximum number of aborted nodes that can be stored in the
    /// driver's memory. If this value is exceeded, the driver will collect garbage.
    pub gc_threshold: usize,

    /// Set channel capacity. This value is used to initialize the channel that connects the driver
    /// and its handles. If the channel is full, the driver will block until the channel is
    /// available.
    ///
    /// When [`None`] is specified, an unbounded channel will be used.
    #[setters(into)]
    pub channel_capacity: Option<usize>,

    /// A shared handle to represent naive number of garbage nodes to collect
    #[setters(skip)]
    gc_counter: Arc<AtomicIsize>,

    // Force 'default' only
    #[setters(skip)]
    _0: (),
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            schedule_resolution: DEFAULT_SCHEDULE_RESOLUTION,
            gc_threshold: 1000,
            channel_capacity: None,
            gc_counter: Default::default(),
            _0: (),
        }
    }
}

impl Builder {
    pub fn build(self) -> (Handle, impl FnOnce()) {
        self.build_d_ary::<2>()
    }

    pub fn build_d_ary<const D: usize>(self) -> (Handle, impl FnOnce()) {
        let _ = instant::origin(); // Force initialization of the global pivot time

        let (tx, rx) = if let Some(cap) = self.channel_capacity {
            channel::bounded(cap)
        } else {
            channel::unbounded()
        };

        let handle = Handle { tx: tx.clone(), gc_counter: self.gc_counter.clone() };
        let driver = move || driver::execute::<D>(self, rx);

        (handle, driver)
    }
}

pub fn create() -> (Handle, impl FnOnce()) {
    Builder::default().build()
}

pub fn create_d_ary<const D: usize>() -> (Handle, impl FnOnce()) {
    Builder::default().build_d_ary::<D>()
}

/* ------------------------------------------- Driver ------------------------------------------- */
mod driver {
    use std::{
        cell::UnsafeCell,
        sync::{atomic::Ordering, Weak},
        task::Waker,
        time::{Duration, Instant},
    };

    use crossbeam::channel::{self, TryRecvError};
    use dary_heap::DaryHeap;
    use educe::Educe;

    use crate::Builder;

    #[derive(Debug)]
    pub(crate) enum Event {
        SleepUntil(NodeDesc),
    }

    pub(crate) fn execute<const D: usize>(this: Builder, rx: channel::Receiver<Event>) {
        let mut nodes = DaryHeap::<Node, D>::new();
        let pivot = Instant::now();
        let to_usec = |x: Instant| x.duration_since(pivot).as_micros() as u64;
        let resolution_usec = this.schedule_resolution.as_micros() as u64;

        // As each node always increment the `gc_counter` by 1 when dropped, and the worker
        // decrements the number by 1 when a node is cleared, this value is expected to be a naive
        // status of 'aborted but not yet handled' node count, i.e. garbage nodes.
        let gc_counter = this.gc_counter;

        'worker: loop {
            let now = to_usec(Instant::now());
            let mut event = if let Some(node) = nodes.peek() {
                let remain = node.timeout_usec.saturating_sub(now);
                if remain > resolution_usec {
                    let system_sleep_for = remain - resolution_usec;
                    let Ok(x) = rx.recv_timeout(Duration::from_micros( system_sleep_for)) else { continue };
                    x
                } else {
                    'busy_wait: loop {
                        let now = to_usec(Instant::now());
                        if now >= node.timeout_usec {
                            let node = nodes.pop().unwrap();

                            if let Some(waker) = node.weak_waker.upgrade() {
                                // SAFETY: No other thread access value of this pointer.
                                unsafe { (*waker.get()).take().unwrap().wake() }
                            }

                            let n_garbage = gc_counter.fetch_sub(1, Ordering::Release);
                            if n_garbage > this.gc_threshold as isize {
                                let n_collect = gc(&mut nodes) as _;
                                gc_counter.fetch_sub(n_collect, Ordering::Release);
                            }

                            continue 'worker;
                        } else {
                            match rx.try_recv() {
                                Ok(x) => break 'busy_wait x,
                                Err(TryRecvError::Disconnected) if nodes.is_empty() => {
                                    break 'worker
                                }
                                Err(TryRecvError::Disconnected) | Err(TryRecvError::Empty) => {
                                    std::thread::yield_now();
                                    continue 'busy_wait;
                                }
                            }
                        }
                    }
                }
            } else {
                let Ok(x) = rx.recv() else {
                    break
                };
                x
            };

            if gc_counter.load(Ordering::Acquire) as usize > this.gc_threshold {
                let n_collect = gc(&mut nodes) as _;
                gc_counter.fetch_sub(n_collect, Ordering::Release);
            }

            'flush: loop {
                match event {
                    Event::SleepUntil(desc) => nodes
                        .push(Node { timeout_usec: to_usec(desc.timeout), weak_waker: desc.waker }),
                };

                event = match rx.try_recv() {
                    Ok(x) => x,
                    Err(TryRecvError::Disconnected) if nodes.is_empty() => break 'worker,
                    Err(TryRecvError::Disconnected) | Err(TryRecvError::Empty) => break 'flush,
                };
            }
        }

        assert!(nodes.is_empty());
        assert_eq!(gc_counter.load(Ordering::Relaxed), 0);
    }

    fn gc<const D: usize>(nodes: &mut DaryHeap<Node, D>) -> usize {
        let fn_retain = |x: &Node| x.weak_waker.upgrade().is_some();

        let prev_len = nodes.len();

        *nodes = {
            let mut vec = std::mem::take(nodes).into_vec();
            vec.retain(fn_retain);
            DaryHeap::from(vec)
        };

        let n_collected = prev_len - nodes.len();
        n_collected
    }

    #[derive(Debug, Clone)]
    pub(crate) struct NodeDesc {
        pub timeout: Instant,
        pub waker: Weak<WakerCell>,
    }

    #[derive(Debug, Clone, Educe)]
    #[educe(Eq, PartialEq, PartialOrd, Ord)]
    pub(crate) struct Node {
        #[educe(PartialOrd(method = "cmp_rev_partial"), Ord(method = "cmp_rev"))]
        pub timeout_usec: u64,

        #[educe(Eq(ignore), PartialEq(ignore), PartialOrd(ignore), Ord(ignore))]
        pub weak_waker: Weak<WakerCell>,
    }

    fn cmp_rev(a: &u64, b: &u64) -> std::cmp::Ordering {
        b.cmp(a)
    }

    fn cmp_rev_partial(a: &u64, b: &u64) -> Option<std::cmp::Ordering> {
        b.partial_cmp(a)
    }

    #[derive(Debug)]
    pub(crate) struct WakerCell {
        // SAFETY: This UnsafeCell is only used to take the value out of the waker.
        //  dsa sads
        waker: UnsafeCell<Option<Waker>>,
    }

    unsafe impl Sync for WakerCell {}
    unsafe impl Send for WakerCell {}

    impl std::ops::Deref for WakerCell {
        type Target = UnsafeCell<Option<Waker>>;

        fn deref(&self) -> &Self::Target {
            &self.waker
        }
    }

    impl WakerCell {
        pub fn new(waker: Waker) -> Self {
            Self { waker: UnsafeCell::new(Some(waker)) }
        }
    }
}

/* ------------------------------------------- Handle ------------------------------------------- */
#[derive(Debug, Clone)]
pub struct Handle {
    tx: channel::Sender<driver::Event>,
    gc_counter: Arc<AtomicIsize>,
}

impl Handle {
    /// Returns a future that sleeps for the specified duration.
    ///
    /// [`SleepFuture`] returns the duration that overly passed the specified duration.
    pub fn sleep_for(&self, duration: Duration) -> SleepFuture {
        self.sleep_until(Instant::now() + duration)
    }

    /// Returns a future that sleeps until the specified instant.
    ///
    /// [`SleepFuture`] returns the duration that overly passed the specified instant.
    pub fn sleep_until(&self, timeout: Instant) -> SleepFuture {
        SleepFuture {
            tx: self.tx.clone(),
            state: SleepState::Pending,
            timeout,
            gc_counter: self.gc_counter.clone(),
        }
    }

    /// Create an interval controller which wakes up after specified `interval` duration on
    /// every call to [`util::Interval::wait`]
    pub fn interval(&self, interval: Duration) -> util::Interval {
        util::Interval { handle: self.clone(), wakeup_time: Instant::now() + interval, interval }
    }
}

pub mod util {
    use crate::{instant, Report};
    use std::time::{Duration, Instant};

    /// Interval controller.
    #[derive(Debug, Clone)]
    pub struct Interval {
        pub(crate) handle: super::Handle,
        pub(crate) wakeup_time: Instant,
        pub(crate) interval: Duration,
    }

    impl Interval {
        /// Wait until next interval.
        ///
        /// This function will return [`SleepResult`] which contains the duration that overly passed
        /// the specified interval. As it internally aligns to the specified interval, it should not
        /// be drifted over time, in terms of [`Instant`] clock domain.
        ///
        /// - `minimum_interval`: Minimum interval to wait. This prevents burst after long
        ///   inactivity on `Interval` object.
        pub async fn tick_with_min_interval(
            &mut self,
            minimum_interval: Duration,
        ) -> Result<Report, crate::Error> {
            assert!(minimum_interval <= self.interval);
            let Self { handle, wakeup_time: wakeup, interval } = self;

            let result = handle.sleep_until(*wakeup).await;
            let now = Instant::now();
            *wakeup = *wakeup + *interval;

            let minimum_next = now + minimum_interval;
            if minimum_next > *wakeup {
                // XXX: We use 128 bit integer to avoid overflow in nanosecond domain.
                //  This is not a perfect solution, but it should be enough for most cases,
                //  as 'over-sleep' is relatively rare case thus slow path is not a big deal.
                let interval_ns = interval.as_nanos();
                let num_ticks = ((minimum_next - *wakeup).as_nanos() - 1) / interval_ns + 1;

                // Set next wakeup to nearest aligned timestamp.
                *wakeup += Duration::from_nanos((interval_ns * num_ticks) as _);
            }

            result
        }

        /// A shortcut for [`tick_with_min_interval`] with `minimum_interval` set to half.
        pub async fn tick(&mut self) -> Result<Report, crate::Error> {
            self.tick_with_min_interval(self.interval / 2).await
        }

        /// Reset interval to the specified duration.
        pub fn set_interval(&mut self, interval: Duration) {
            assert!(interval > Duration::default());
            self.wakeup_time -= self.interval;
            self.wakeup_time += interval;
            self.interval = interval;
        }

        pub fn interval(&self) -> Duration {
            self.interval
        }

        pub fn wakeup_time(&self) -> Instant {
            self.wakeup_time
        }

        /// This method aligns the subsequent tick to a given interval. Following the alignment, the
        /// timestamp will conform to the specified interval.
        ///
        /// Parameters:
        /// - `now_since_epoch`: A function yielding the current time since the epoch. It's
        ///   internally converted to an [`Instant`], hence, should return the most recent
        ///   timestamp.
        /// - `align_offset_ns`: This parameter can adjust the alignment timing. For example, an
        ///   offset of 100us applied to a next tick scheduled at 9000us will push the tick to
        ///   9100us.
        /// - `initial_interval_tolerance`: This defines the permissible noise level for the initial
        ///   interval. If set to zero, the actual sleep duration will exceed the interval,
        ///   potentially causing a tick to be skipped as the actual sleep duration might be twice
        ///   the interval. It's advisable to set it to 10% of the interval to prevent the
        ///   `align_clock` command from disrupting the initial interval.
        ///
        /// Note: For example, if `now_since_epoch()` gives 8500us and the interval is 1000us, the
        /// subsequent tick will be adjusted to 9000us, aligning with the interval.
        pub fn align_with_clock(
            &mut self,
            now_since_epoch: impl FnOnce() -> Duration,
            interval: Option<Duration>, // If none, reuse the previous interval.
            initial_interval_tolerance: Option<Duration>, // If none, 10% of the interval.
            align_offset_ns: i64,
        ) {
            let prev_trig = self.wakeup_time - self.interval;
            let dst_now_ns = now_since_epoch().as_nanos() as i64;
            let inst_now = Instant::now();

            let interval = interval.unwrap_or(self.interval);
            let interval_ns = interval.as_nanos() as i64;
            let interval_tolerance =
                initial_interval_tolerance.unwrap_or(Duration::from_nanos((interval_ns / 10) as _));

            assert!(interval > Duration::default(), "interval must be larger than zero");
            assert!(interval_tolerance < interval);

            let ticks_to_align = {
                let mut val = interval_ns - (dst_now_ns % interval_ns) + align_offset_ns;
                if val < 0 {
                    val += (val / interval_ns + 1) * interval_ns;
                }
                Duration::from_nanos((val % interval_ns) as _)
            };

            let mut desired_wake_up = inst_now + ticks_to_align;
            if desired_wake_up < prev_trig + interval - interval_tolerance {
                desired_wake_up += interval;
                debug_assert!(desired_wake_up >= prev_trig + interval - interval_tolerance);
            }

            self.wakeup_time = desired_wake_up;
            self.interval = interval;
        }

        /// Shortcut for [`Self::align_clock`] from now.
        pub fn align_now(
            &mut self,
            interval: Option<Duration>,
            initial_interval_tolerance: Option<Duration>,
            align_offset_ns: i64,
        ) {
            self.align_with_clock(
                || instant::time_from_epoch(),
                interval,
                initial_interval_tolerance,
                align_offset_ns,
            );
        }

        /// Shortcut for [`Self::align_clock`] with [`std::time::SystemTime`] as the time source.
        #[cfg(feature = "system-clock")]
        pub fn align_with_system_clock(
            &mut self,
            interval: Option<Duration>,
            initial_interval_tolerance: Option<Duration>,
            align_offset_ns: i64,
        ) {
            self.align_with_clock(
                || {
                    let now = std::time::SystemTime::now();
                    now.duration_since(std::time::UNIX_EPOCH).unwrap()
                },
                interval,
                initial_interval_tolerance,
                align_offset_ns,
            );
        }
    }
}

mod instant {
    use std::time::Instant;

    pub(crate) fn origin() -> Instant {
        lazy_static::lazy_static!(
            static ref PIVOT: Instant = Instant::now();
        );

        *PIVOT
    }

    pub(crate) fn time_from_epoch() -> std::time::Duration {
        Instant::now().duration_since(origin())
    }
}

/* ------------------------------------------- Future ------------------------------------------- */
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SleepFuture {
    tx: channel::Sender<driver::Event>,
    gc_counter: Arc<AtomicIsize>,
    timeout: Instant,
    state: SleepState,
}

#[cfg(test)]
static_assertions::assert_impl_all!(SleepFuture: Send, Sync, Unpin);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("driver shutdown")]
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Report {
    /// Timer has been correctly requested, and woke up normally. Returned value is overslept
    /// duration than the requested timeout.
    Completed(Duration),

    /// Timer has not been requested as the timeout is already expired.
    ExpiredTimer(Duration),
}

impl Report {
    pub fn overslept(&self) -> Duration {
        match self {
            Self::Completed(dur) => *dur,
            Self::ExpiredTimer(dur) => *dur,
        }
    }
}

#[derive(Debug)]
enum SleepState {
    Pending,
    Sleeping(Arc<WakerCell>),
    Woken,
}

unsafe impl Sync for SleepState {}
unsafe impl Send for SleepState {}

impl std::future::Future for SleepFuture {
    type Output = Result<Report, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = Instant::now();

        if let Some(over) = now.checked_duration_since(self.timeout) {
            let result = if matches!(self.state, SleepState::Sleeping(_)) {
                self.state = SleepState::Woken;
                Report::Completed(over)
            } else {
                Report::ExpiredTimer(over)
            };

            return Poll::Ready(Ok(result));
        }

        if matches!(self.state, SleepState::Pending) {
            let waker = Arc::new(WakerCell::new(cx.waker().clone()));
            let event = driver::Event::SleepUntil(NodeDesc {
                timeout: self.timeout,
                waker: Arc::downgrade(&waker),
            });

            if let Err(_) = self.tx.send(event) {
                return Poll::Ready(Err(Error::Shutdown));
            }

            self.state = SleepState::Sleeping(waker);
        }

        Poll::Pending
    }
}

impl Drop for SleepFuture {
    fn drop(&mut self) {
        if !matches!(&self.state, SleepState::Pending) {
            self.gc_counter.fetch_add(1, Ordering::Release);
        }

        // eprint!("{}          \r", self.gc_counter.load(Ordering::Relaxed));
    }
}
