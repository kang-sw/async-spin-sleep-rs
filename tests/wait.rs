use futures::future::join_all;

#[tokio::test]
async fn verify_api() {
    let init = async_spin_sleep::Init::default();
    let handle = init.handle();

    std::thread::spawn(move || init.execute());
    for sleep in
        join_all((0..100).rev().map(|i| handle.sleep_for(std::time::Duration::from_millis(i))))
            .await
    {
        println!("{sleep:?}");
    }
}
