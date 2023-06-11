use futures::future::join_all;

#[tokio::test]
async fn verify_api() {
    let init = async_spin_sleep::Init::default();
    let handle = init.handle();

    std::thread::spawn(move || init.blocking_execute());
    for sleep in join_all(
        (0..100).rev().map(|i| handle.sleep_for(std::time::Duration::from_micros(i) * 150)),
    )
    .await
    {
        println!("{sleep:?}");
    }
}

#[tokio::test]
async fn discard_half() {
    let mut init = async_spin_sleep::Init::default();
    init.collect_garbage_at = 3;
    let handle = init.handle();

    std::thread::spawn(move || init.blocking_execute());
    for sleep in join_all((0..100).rev().map(|i| {
        let handle = handle.clone();
        async move {
            tokio::select! {
              a = handle.sleep_for(std::time::Duration::from_micros(i) * 150) => a,
              b = handle.sleep_for(std::time::Duration::from_micros(i) * 300) => b,
            }
        }
    }))
    .await
    {
        println!("{sleep:?}");
    }
}
