# async-spin-sleep

**async-spin-sleep** is a Rust crate that provides a highly accurate asynchronous timer for multiple async tasks using spin sleep in a single thread.

## MSRV

As this crate internally utilizes `BinaryHeap::retain` method, it requires rust compiler version 1.70.0

## Usage

To use **async-spin-sleep**, add the following line to your `Cargo.toml` file:

```toml
[dependencies]
async-spin-sleep = "{choose-version-here}"
```

Then, in your Rust code, you can use it as follows:

```rust
use async_spin_sleep::{Init, join_all};

#[tokio::main]
async fn main() {
    let init = Init::default();
    let handle = init.handle();

    std::thread::spawn(move || init.execute());
    for sleep in join_all(
        (0..100).rev().map(|i| handle.sleep_for(std::time::Duration::from_micros(i) * 150)),
    )
    .await
    {
        println!("{sleep:?}");
    }
}
```

In this example, an `Init` instance is created, and its handle is obtained. A separate thread is spawned to execute the initialization. Then, a loop is used to schedule multiple asynchronous sleep operations using the `sleep_for` method of the handle. The sleep durations are calculated based on the iteration index. Finally, the results of the sleep operations are printed.

Make sure to use the `#[tokio::main]` attribute if you're using Tokio as your async runtime.

## Features

- Provides highly accurate asynchronous timers for multiple async tasks.
- Uses spin sleep to minimize overhead and provide precise timing.
- Designed for usage in single-threaded environments.

## License

This crate is distributed under the terms of the MIT license. See [LICENSE](LICENSE) for details.

## Contributing

Contributions in the form of bug reports, pull requests, or general feedback are welcome. Feel free to open an issue or submit a pull request on the [GitHub repository](https://github.com/kang-sw/async-spin-sleep-rs).
