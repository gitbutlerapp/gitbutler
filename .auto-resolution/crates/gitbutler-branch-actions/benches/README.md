To run benchmarks, add the `benches` feature.

```shell
cargo bench --features benches
```

It's used to get past a safety check in `gitbutler-git`.

For faster compile times, specify the benchmark file directly, i.e.

```shell
cargo bench --bench branches --features benches
```
