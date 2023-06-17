# Async spin sleep

고해상도 비동기 타이머 드라이버

`async-spin-sleep`은 `spin loop`를 통해 구현된 고해상도 타이머의 이점을, 많은 수의 비동기 태스크에
적은 오버헤드로 제공합니다.

```rust
    use futures::executor::block_on;
    use std::time::{Duration, Instant};

    let (handle, driver) = async_spin_sleep::create();
    std::thread::spawn(driver);

    block_on(async {
        let begin = Instant::now();
        let overslept = handle
            .sleep_for(Duration::from_millis(100))
            .await
            .expect("sleep function always return Ok(...), as long as the driver thread alive");

        assert!(begin.elapsed() >= Duration::from_millis(100));
        println!("t: {:?}, overslept: {:?}", begin.elapsed(), overslept);
    });
```

# Why is this useful?

하드웨어 시스템과 통신하거나 미디어 출력 등을 구현할 때, 일반적인 PC 하드웨어에서 리얼 타임에 준하는
시스템을 구현해야 하는 경우가 있습니다. 윈도우 OS의 타이머 해상도는 일반적으로 20ms 수준으로 알려져
있으며, 이는 시간에 민감한 어플리케이션에서 많은 경우 불충분한 것으로 간주됩니다.

따라서, 시간 제약을 극복하기 위해 일반적으로 `spin-loop`가 사용되는데, 이는 하나의 CPU 코어를 지정된
시간 동안 100% 점유하며, 점유한 CPU 시간은 오로지 시간을 측정하는데만 사용됩니다. 만약 여러 개의
컨텍스트에서 동일한 시간 제약을 요구할 때, 여러 문맥에서 동시에 `spin-loop`을 사용하는 것은
시스템 전체에 심각한 부하를 줄 수 있습니다.

`async-spin-sleep`의 기본적인 아이디어는, `spin-loop`를 사용하는 데디케이티드 타이머 드라이버를
하나의 스레드에서만 실행하고, 정확한 타이밍에 비동기 태스크를 wakeup시키는 것입니다. 하나의 코어가
적어도 운영체제의 스케쥴러 해상도보다 낮은 시간 동안은 CPU를 100% 점유해야 한다는 점은 일반적인 동기
`spin-loop`과 같지만, 많은 수의 비동기 태스크가 `spin-loop`의 이점을 공유할 수 있습니다.

# Usage

[문서화](https://docs.rs/async-spin-sleep) 참조.

# License

Licensed under 

- MIT license ([LICENSE-MIT](LICENSE) or <http://opensource.org/licenses/MIT>)
