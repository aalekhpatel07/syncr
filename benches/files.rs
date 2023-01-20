use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use pprof::criterion::{Output, PProfProfiler};
use rand::{thread_rng, RngCore};
use syncr::weak_checksum::*;

pub fn bench_weak_checksum(c: &mut Criterion) {
    let mut group = c.benchmark_group("weak_checksum");
    for length in (0..=1_000_000).step_by(100_000) {
        group.throughput(Throughput::Bytes(length as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(length),
            &length,
            |b, &length| {
                let mut buffer = vec![0u8; length];
                let mut rng = thread_rng();

                b.iter(|| {
                    rng.fill_bytes(&mut buffer);
                    RollingCheckSum::new(&buffer)
                        .rolling_checksums()
                        .for_each(drop);
                });
            },
        );
    }
    group.finish();
}

criterion_main!(benches);

criterion_group! {
    name = benches;
    config =
        Criterion::default()
        .with_profiler(
            PProfProfiler::new(100, Output::Flamegraph(None))
        );
    targets = bench_weak_checksum
}
