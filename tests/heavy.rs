use std::{
    sync::{atomic::AtomicU64, Arc},
    time::{Duration, Instant},
};

use lazy_static::lazy_static;

#[test]
fn heavy_use() {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(async {
        let (handle, worker) = async_spin_sleep::create();
        std::thread::spawn(worker);

        const NUM_TASKS: usize = 50;
        const CHECK_TICKS: usize = 1000;
        let time_check_refs = (0..NUM_TASKS)
            .map(|_| {
                let updator = Arc::new(AtomicU64::new(now_us()));
                let handle = handle.clone();

                tokio::spawn({
                    let updator = updator.clone();
                    async move {
                        loop {
                            handle.sleep_for(Duration::from_millis(10)).await;
                            updator.store(now_us(), std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });

                updator
            })
            .collect::<Vec<_>>();

        for seq in 0..CHECK_TICKS {
            // Sleep 10ms between each task
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Check if there's any task that has not been updated for 100ms
            let mut num_ok = 0;
            for updator in &time_check_refs {
                let last_update = updator.load(std::sync::atomic::Ordering::Relaxed);
                let time_diff = now_us() - last_update;

                if time_diff < 500_000 {
                    num_ok += 1;
                }
            }

            assert_eq!(num_ok, NUM_TASKS, "Some tasks are not updated for 500ms");
            eprint!("{seq:8} / {CHECK_TICKS:8} \r")
        }

        ();
    })
}

fn now_us() -> u64 {
    lazy_static! {
        static ref PIVOT: Instant = Instant::now();
    };

    PIVOT.elapsed().as_micros() as u64
}
