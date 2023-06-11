//! This library provides a timer driver for scheduling and executing timed operations. The driver
//! allows you to create handles for sleeping for a specific duration or until a specified timeout.
//! It operates in a concurrent environment and uses a binary heap for efficient scheduling of
//! events.
//!
//! # Example
//!
//! ```rust
//! use async_spin_sleep::Init;
//! use std::time::Duration;
//!
//! // Initialize the timer driver
//! let timer = Init::default();
//!
//! // Create a handle for sleeping for 1 second
//! let handle = timer.handle();
//!
//! // Spawn the driver on a separate thread.
//! // The timer will be dropped when all handles are dropped.
//! std::thread::spawn(move || timer.blocking_execute());
//!
//! let sleep_future = handle.sleep_for(Duration::from_secs(1));
//!
//! // Wait for the sleep future to complete
//! let result = futures::executor::block_on(sleep_future);
//! if let Ok(overly) = result {
//!     println!("Slept {overly:?} more than requested");
//! } else {
//!     println!("Sleep error: {:?}", result.err());
//! }
//! ```
use std::{
    pin::Pin,
    sync::{atomic::AtomicUsize, mpsc},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use enum_as_inner::EnumAsInner;

/* -------------------------------------------- Init -------------------------------------------- */
#[cfg(windows)]
const DEFAULT_SCHEDULE_RESOLUTION: Duration = Duration::from_millis(33);
#[cfg(unix)]
const DEFAULT_SCHEDULE_RESOLUTION: Duration = Duration::from_millis(3);

#[derive(Debug, typed_builder::TypedBuilder)]
pub struct Init {
    /// Default scheduling resolution for this driver. Setting this to a lower value may decrease
    /// CPU usage of the driver, but may also dangerously increase the chance of missing a wakeup
    /// event.
    #[builder(default = DEFAULT_SCHEDULE_RESOLUTION)]
    pub schedule_resolution: Duration,

    /// Aborted nodes that are too far from execution may remain in the driver's memory for a long
    /// time. This value specifies the maximum number of aborted nodes that can be stored in the
    /// driver's memory. If this value is exceeded, the driver will collect garbage.
    #[builder(default = 128)]
    pub collect_garbage_at: usize,

    // For internal use
    #[builder(default = Some(mpsc::channel()), setter(skip))]
    channel: Option<(mpsc::Sender<driver::Event>, mpsc::Receiver<driver::Event>)>,
}

impl Default for Init {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Init {
    /// Creates a handle to the timer driver.
    pub fn handle(&self) -> Handle {
        Handle { tx: self.channel.as_ref().unwrap().0.clone() }
    }

    /// Blocks current thread, executing the driver.
    pub fn blocking_execute(mut self) {
        let (_, rx) = self.channel.take().unwrap();
        driver::execute(self, rx)
    }
}

/* ------------------------------------------- Driver ------------------------------------------- */
mod driver {
    use std::{
        collections::{BTreeSet, BinaryHeap},
        sync::mpsc::{self, TryRecvError},
        task::Waker,
        time::Instant,
    };

    use educe::Educe;

    use crate::Init;

    #[derive(Debug)]
    pub(crate) enum Event {
        SleepUntil(NodeDesc, Waker),
        Abort(NodeDesc),
    }

    /// ```plain
    /// if exists foremost-node
    ///     if node is far from execution
    ///         condvar-sleep until safety limit
    ///         continue
    ///     else
    ///         while until foremost-node is executed
    /// else
    ///     wait condvar
    /// ```
    pub(crate) fn execute(this: Init, rx: mpsc::Receiver<Event>) {
        let mut nodes = BinaryHeap::<Node>::new();
        let mut aborts = BTreeSet::<usize>::new();
        let mut n_garbage = 0usize;
        let mut cursor_timeout = Instant::now(); // prevents expired node abortion

        'outer: loop {
            let now = Instant::now();

            let event = if let Some(node) = nodes.peek() {
                let remain = node.desc.timeout.saturating_duration_since(now);
                if remain > this.schedule_resolution {
                    let system_sleep_for = remain - this.schedule_resolution;
                    let Ok(x) = rx.recv_timeout(system_sleep_for) else { continue };
                    x
                } else {
                    loop {
                        let now = Instant::now();
                        if now >= node.desc.timeout {
                            // This is the only point where a node is executed.
                            let node = nodes.pop().unwrap();

                            if let Some(_) = aborts.take(&node.desc.id) {
                                n_garbage -= 1;
                            } else {
                                node.waker.wake();
                            }

                            cursor_timeout = node.desc.timeout;
                            continue 'outer;
                        } else {
                            match rx.try_recv() {
                                Ok(x) => break x,
                                Err(TryRecvError::Empty) => std::thread::yield_now(),
                                Err(TryRecvError::Disconnected) => break 'outer,
                            }
                        }
                    }
                }
            } else {
                let Ok(x) = rx.recv() else { break };
                x
            };

            match event {
                Event::SleepUntil(desc, waker) => nodes.push(Node { waker, desc }),

                Event::Abort(node) if node.timeout > cursor_timeout => {
                    aborts.insert(node.id);
                    n_garbage += 1;

                    if n_garbage > this.collect_garbage_at {
                        let fn_retain = |x: &Node| {
                            if let Some(_) = aborts.take(&x.desc.id) {
                                n_garbage -= 1;
                                false
                            } else {
                                true
                            }
                        };

                        #[rustversion::since(1.70)]
                        fn retain(
                            this: &mut BinaryHeap<Node>,
                            fn_retain: impl FnMut(&Node) -> bool,
                        ) {
                            this.retain(fn_retain);
                        }

                        #[rustversion::before(1.70)]
                        fn retain(
                            this: &mut BinaryHeap<Node>,
                            fn_retain: impl FnMut(&Node) -> bool,
                        ) {
                            let mut vec = nodes.into_vec();
                            vec.retain(fn_retain);
                            nodes = BinaryHeap::from(vec);
                        }

                        retain(&mut nodes, fn_retain);

                        debug_assert!(n_garbage == 0);
                        debug_assert!(aborts.is_empty());
                    }
                }

                Event::Abort(_) => (), // It is safe to ignore.
            }
        }
    }

    #[derive(Debug, Eq, PartialEq, Clone, Copy, Educe)]
    #[educe(PartialOrd, Ord)]
    pub(crate) struct NodeDesc {
        #[educe(PartialOrd(method = "cmp_rev_partial"), Ord(method = "cmp_rev"))]
        pub timeout: Instant,
        pub id: usize,
    }

    fn cmp_rev(a: &Instant, b: &Instant) -> std::cmp::Ordering {
        b.cmp(a)
    }

    fn cmp_rev_partial(a: &Instant, b: &Instant) -> Option<std::cmp::Ordering> {
        b.partial_cmp(a)
    }

    #[derive(Debug, Educe)]
    #[educe(PartialEq, Eq, PartialOrd, Ord)]
    struct Node {
        #[educe(PartialEq(ignore), Eq(ignore), PartialOrd(ignore), Ord(ignore))]
        waker: Waker,
        desc: NodeDesc,
    }
}

/* ------------------------------------------- Handle ------------------------------------------- */
#[derive(Debug, Clone)]
pub struct Handle {
    tx: mpsc::Sender<driver::Event>,
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
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        SleepFuture {
            tx: self.tx.clone(),
            state: SleepState::Pending,
            desc: driver::NodeDesc {
                timeout,
                id: COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            },
        }
    }
}

/* ------------------------------------------- Future ------------------------------------------- */
#[derive(Debug)]
pub struct SleepFuture {
    tx: mpsc::Sender<driver::Event>,
    desc: driver::NodeDesc,
    state: SleepState,
}

#[derive(Debug, thiserror::Error)]
pub enum SleepError {
    #[error("driver shutdown")]
    Shutdown,
}

#[derive(Debug, EnumAsInner)]
enum SleepState {
    Pending,
    Sleeping,
    Woken,
}

impl std::future::Future for SleepFuture {
    type Output = Result<Duration, SleepError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = Instant::now();

        if let Some(over) = now.checked_duration_since(self.desc.timeout) {
            self.state = SleepState::Woken;
            return Poll::Ready(Ok(over));
        }

        if self.state.is_pending() {
            let event = driver::Event::SleepUntil(self.desc, cx.waker().clone());
            if let Err(_) = self.tx.send(event) {
                return Poll::Ready(Err(SleepError::Shutdown));
            }

            self.state = SleepState::Sleeping;
        }

        Poll::Pending
    }
}

impl Drop for SleepFuture {
    fn drop(&mut self) {
        if self.state.is_sleeping() {
            let _ = self.tx.send(driver::Event::Abort(self.desc));
        }
    }
}
