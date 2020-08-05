A simple benchmark exploring a messege passing applicaiton and the performance impact of threads and
different async runtimes.


The setup consists of 3 phases:

1) generate data (parse a JSON)
2) do something with the data (we calculate the length of a string in the object and store it)
3) serialize and count data (we serialize the data and count it's length to be able to calculate average throughput)


The first argument is the concurrency of step 1 and 2 so a concurrency of 1 looks like this:

```
IN -> PROCESS -> OUT
```

a concurrency of 2 looks like this:
```
IN -> PROCESS \
               -> OUT
IN -> PROCESS /

```
cargo build --all --release && strip target/release/runtime-bench && strip target/release/thread-bench
for i in 1 2 4 8 16 32 64 128
do
  taskset -c 0,1,2 ./target/release/runtime-bench $i
  ./target/release/runtime-bench $i
done
for i in 1 2 4 8 16 32 64 128
do
  taskset -c 0,1,2 ./target/release/thread-bench $i
  ./target/release/thread-bench $i
done
```