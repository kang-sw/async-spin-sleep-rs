//! Code segments to write doc

#[cfg(test)]
#[test]
fn doc_readme_1() {
    use futures::executor::block_on;
    use std::time::{Duration, Instant};

    let (handle, driver) = async_spin_sleep::create();
    std::thread::spawn(driver);

    block_on(async {
        let begin = Instant::now();
        let overslept = handle.sleep_for(Duration::from_millis(100)).await;

        assert!(begin.elapsed() >= Duration::from_millis(100));
        println!("t: {:?}, overslept: {:?}", begin.elapsed(), overslept);
    });
}
