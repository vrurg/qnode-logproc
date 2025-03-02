# Running

```
./generator.sh | cargo run
```

# Disclaimer

This is a later version which I completed over a few spare hours I had lately. The version submitted for the test
is available at the `final` tag (`git checkout final`).

# Explanations

With Rust’s capabilities, there’s no need to adjust either the input buffer or the incoming queue buffer. The buffered
read itself is fast enough (tested using `cat file | cargo run`). The processing queue is managed by Tokio’s `mpsc`
channel, which gives us direct control. Additionally, to avoid losing any log entries, all pre-parsed packets must be
retained, making it impossible to reduce memory usage unless the processing of the internal records buffer is
sufficiently fast.

Because the provided log generation tool doesn’t produce log entries quickly enough, it’s challenging to test burst
handling. However, measuring per-second rates or setting up a simple threshold detection is straightforward. Given the
limited spare time, I decided not to focus on that aspect. Also, I had to intentionally omit tests.

Eventually, the project’s primary focus shifted slightly to the following requirements:

1. Code quality
2. Management of sliding windows
3. Weighted rates
4. Memory management

I'm not completely satisfied with the code quality—time constraints prevented me from refactoring it as thoroughly as I
would have liked. Improvements in encapsulation, delegation of responsibilities, and error management are still needed.

The other two requirements presented interesting challenges. In particular, weighted rates were previously implemented
under a completely different set of requirements, so this change serves as a valuable exercise.

For memory management, the focus is on ensuring timely cleanup and maintaining a dedicated index for incoming messages.
Instead of handling strings directly, they are mapped to integer indices and, when necessary, mapped back to the
original messages.

Overall, the tool's output is very close to the expected results, though fully testing its capabilities would require
developing a custom log generator.
