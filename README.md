# async-spin-sleep

**async-spin-sleep** is a Rust crate that provides a highly accurate asynchronous timer for multiple async tasks using spin sleep in a single thread.

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

## Improving timer stability

To improve the timer stability of the `init.execute()` thread, you can increment its thread priority. This can be achieved by using platform-specific functions or libraries to adjust the priority level. Here's an example code snippet demonstrating how to increment the thread priority:

```rust
use std::thread;
use async_spin_sleep::Init;

#[cfg(target_os = "windows")]
fn increase_thread_priority(thread: thread::Thread, priority: u32) {
    // Use Windows-specific code to increase the thread priority
    // e.g., using the SetThreadPriority function
}

#[cfg(target_os = "linux")]
fn increase_thread_priority(thread: thread::Thread, priority: u32) {
    // Use Linux-specific code to increase the thread priority
    // e.g., using the sched_setscheduler function
}

#[cfg(target_os = "macos")]
fn increase_thread_priority(thread: thread::Thread, priority: u32) {
    // Use macOS-specific code to increase the thread priority
    // e.g., using the pthread_set_qos_class_np function
}

fn main() {
    let init = Init::default();

    std::thread::spawn(move || {
        increase_thread_priority(thread::current(), MAX);
        init.execute();
    });

    // Rest of your code...
}
```

In this example, platform-specific functions are used inside the `increase_thread_priority` function
to increase the priority of the current thread. You'll need to replace `MAX` with the appropriate
value or constant representing the highest thread priority level for your target platform. Please
consult the platform-specific documentation or libraries to find the correct methods for adjusting
thread priority on your specific operating system.
