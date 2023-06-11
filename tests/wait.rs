use std::time::Duration;

#[tokio::test]
async fn verify_unbounded() {
    let args = async_spin_sleep::Builder::default();

    let (handle, driver) = args.build();
    std::thread::spawn(driver);

    let times = futures::future::join_all(
        (0..10000).rev().map(|i| handle.sleep_for(std::time::Duration::from_micros(i) * 150)),
    )
    .await;

    let len = times.len();
    let avg = times.into_iter().map(|x| x.unwrap()).sum::<Duration>() / len as u32;
    println!("avg: {:?}", avg);
}

#[tokio::test]
async fn verify_channel_size() {
    let mut args = async_spin_sleep::Builder::default();
    args.channel_capacity = Some(50);

    let (handle, driver) = args.build();
    std::thread::spawn(driver);

    let times = futures::future::join_all(
        (0..10000).rev().map(|i| handle.sleep_for(std::time::Duration::from_micros(i) * 150)),
    )
    .await;

    let len = times.len();
    let avg = times.into_iter().map(|x| x.unwrap()).sum::<Duration>() / len as u32;
    println!("avg: {:?}", avg);
}

#[tokio::test]
async fn discard_test() {
    discard::<2>(500).await;
    discard::<2>(1000).await;
    discard::<2>(2000).await;
    discard::<2>(4000).await;
    discard::<2>(8000).await;

    println!("--------------------------------------------------");

    discard::<3>(500).await;
    discard::<3>(1000).await;
    discard::<3>(2000).await;
    discard::<3>(4000).await;
    discard::<3>(8000).await;

    println!("--------------------------------------------------");

    discard::<4>(500).await;
    discard::<4>(1000).await;
    discard::<4>(2000).await;
    discard::<4>(4000).await;
    discard::<4>(8000).await;

    println!("--------------------------------------------------");

    discard::<5>(500).await;
    discard::<5>(1000).await;
    discard::<5>(2000).await;
    discard::<5>(4000).await;
    discard::<5>(8000).await;

    println!("--------------------------------------------------");

    discard::<12>(500).await;
    discard::<12>(1000).await;
    discard::<12>(2000).await;
    discard::<12>(4000).await;
    discard::<12>(8000).await;

    println!("--------------------------------------------------");
}

async fn discard<const D: usize>(gc: usize) {
    let mut init = async_spin_sleep::Builder::default();
    init.collect_garbage_at = gc;

    let (handle, driver) = init.build_d_ary::<D>();
    std::thread::spawn(driver);

    let times = futures::future::join_all((0..10000).rev().map(|i| {
        let handle = handle.clone();
        async move {
            tokio::select! {
              a = handle.sleep_for(std::time::Duration::from_micros(i* 50)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 60)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 70)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 80)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 90)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 100)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 110)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 120)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 130)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 140)) => a,
              a = handle.sleep_for(std::time::Duration::from_micros(i* 150)) => a,
            }
            .unwrap()
        }
    }))
    .await;

    let avg = times.iter().sum::<Duration>() / times.len() as u32;
    let max = times.iter().max().unwrap();
    println!("avg: {avg:?} max: {max:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_threads() {
    let (handle, driver) = async_spin_sleep::create_d_ary::<4>();
    std::thread::spawn(driver);

    let tasks = (0..10000).rev().map(|i| {
        let handle = handle.clone();
        tokio::spawn(async move {
            let e = handle.sleep_for(std::time::Duration::from_micros(i) * 150).await.unwrap();
            if e > Duration::from_millis(1) {
                print!(
                    "{sleep:?} in {thread:?}, ",
                    sleep = e,
                    thread = std::thread::current().id()
                );
            }
            e
        })
    });

    let times = futures::future::join_all(tasks).await;
    let len = times.len();
    let avg = times.into_iter().map(|x| x.unwrap()).sum::<Duration>() / len as u32;

    println!("avg: {:?}", avg);
}
