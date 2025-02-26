# Running

```
./generator.sh | cargo run
```

# Disclaimer

It does too little, only the basics were prepared. My mistake was to focus initially on buffering which is not
really necessary, considering the data types used. Since buffer itself is just a list of smart pointers (`String`)
it wouldn't over-consume memory. Worst case scenarios with processing delay of 1sec and a burst of 10k messages
the buffer itself will only hold like ~40Kb itself. Strings allocated, on the other hand, are gonna stay around
until the processing gets unstuck, thus making this memory totally required.

# Explanations

I really havent got to the point of pattern weighting. So, nothing here.

The window management is handled by averaging the number of held records per current window size. If it gets too
high or too low the window is either halved while it's > 15 seconds; or increased by 10sec steps.

Burst handling isn't done either, but it would be based on the per_second_rate field of the `Stats` struct. There is
not much to do left.

Introducing a concurrency bug isn't an easy task with Rust design. The most common problem with Rust concurrency are
dead-locks.

Rendering was planned as a simple printing with help of the `console` crate. The least complex task. I just haven't
got to have data ready for printing.

Malformed messages are currently just printed out with `NO MATCH` prefix. Otherwise no processing.

# Conclusion

This code is mostly a skeleton. I approximate ~1.5-2 hours extra for it to be completed.
