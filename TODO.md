# 0.5.0 - API changes

- [ ] Change sleep future result to infallible
  - Worker thread must not panicked or aborted until all timers are consumed.
  - If so, it should be panicking behavior, not result.

# Fix

- [ ] **_fatal_** No-wakeup in release build
