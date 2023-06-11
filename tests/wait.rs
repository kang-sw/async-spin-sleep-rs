use std::time::Duration;

#[tokio::test]
async fn verify_api() {
    let (handle, driver) = async_spin_sleep::create();
    std::thread::spawn(driver);

    for sleep in futures::future::join_all(
        (0..10000).rev().map(|i| handle.sleep_for(std::time::Duration::from_micros(i) * 300)),
    )
    .await
    {
        let sleep = sleep.unwrap();
        if sleep > Duration::from_millis(1) {
            println!("{sleep:?}");
        }
    }
}

#[tokio::test]
async fn verify_channel_size() {
    let mut args = async_spin_sleep::Builder::default();
    args.channel_capacity = Some(10);

    let (handle, driver) = args.build();
    std::thread::spawn(driver);

    for sleep in futures::future::join_all(
        (0..10000).map(|i| handle.sleep_for(std::time::Duration::from_micros(i) * 150)),
    )
    .await
    {
        let sleep = sleep.unwrap();
        if sleep > Duration::from_millis(1) {
            println!("{sleep:?}");
        }
    }
}

#[tokio::test]
async fn discard() {
    let mut init = async_spin_sleep::Builder::default();
    init.collect_garbage_at = 10;

    let (handle, driver) = init.build();
    std::thread::spawn(driver);

    for sleep in futures::future::join_all((0..1000).map(|i| {
        let handle = handle.clone();
        async move {
            tokio::select! {
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 300)) => a,
            }
        }
    }))
    .await
    {
        let sleep = sleep.unwrap();
        if sleep > Duration::from_millis(1) {
            println!("{sleep:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_threads() {
    let (handle, driver) = async_spin_sleep::create();
    std::thread::spawn(driver);

    let tasks = (0..10000).map(|i| {
        let handle = handle.clone();
        tokio::spawn(async move {
            let e = handle.sleep_for(std::time::Duration::from_micros(i) * 150).await.unwrap();
            if e > Duration::from_millis(1) {
                println!(
                    "{sleep:?} in {thread:?} ",
                    sleep = e,
                    thread = std::thread::current().id()
                );
            }
        })
    });

    futures::future::join_all(tasks).await;
}
