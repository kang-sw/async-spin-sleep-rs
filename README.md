# Async spin sleep

![crates.io](https://img.shields.io/crates/v/async-spin-sleep.svg)

<p style="text-align: center;"><a href="docs/README.ko.md">한국어</a></p>

High-resolution asynchronous spin-timer driver

The `async-spin-sleep` library offers an efficient method to leverage the advantages of a high-resolution timer through a `spin loop`, catering to numerous asynchronous tasks with minimal overhead.

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
            .expect("sleep function always returns Ok(...), as long as the driver thread is alive");

        assert!(begin.elapsed() >= Duration::from_millis(100));
        println!("t: {:?}, overslept: {:?}", begin.elapsed(), overslept);
    });
```

# Why is this useful?

Certain applications, such as real-time system implementations on standard PC hardware, media output, or hardware interaction, often demand high-resolution timers. Conventional operating systems like Windows have a timer resolution of around 20ms, which might be insufficient for time-sensitive applications.

A common solution to this challenge is to employ a `spin-loop`, a busy-wait technique that consumes 100% of a CPU core's time for precise timing measurements. However, when multiple contexts demand similar time precision, simultaneous `spin-loop` usage can significantly stress the system.

`async-spin-sleep` alleviates this issue by running a dedicated timer driver, utilizing a `spin-loop` from a single thread. This thread awakens asynchronous tasks at precise moments. This approach maintains a core fully occupied for a time span shorter than the OS's scheduler resolution, similar to a conventional `spin-loop`, yet enables sharing of this resource among numerous asynchronous tasks.

# Usage

Refer to the [documentation](https://docs.rs/async-spin-sleep).

# License

Licensed under either of the following:

- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
