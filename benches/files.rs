use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use pprof::criterion::{Output, PProfProfiler};
use rand::{thread_rng, RngCore};
use syncr::Checksums;
use syncr::weak_checksum::WeakCheckSum;
use syncr::strong_checksum::StrongCheckSum;

const STEP_SIZE: usize = 200_000;
const MAX_SIZE: usize = 1_000_000;


pub fn bench_strong_checksum_rolling(c: &mut Criterion) {
    let mut group = c.benchmark_group("strong_checksum_rolling");
    for length in (0..=MAX_SIZE).step_by(STEP_SIZE) {
        group.throughput(Throughput::Bytes(length as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(length),
            &length,
            |b, &length| {
                let mut buffer = vec![0u8; length];
                let mut rng = thread_rng();

                b.iter(|| {
                    rng.fill_bytes(&mut buffer);
                    StrongCheckSum::new()
                        .checksums(&buffer)
                        .for_each(drop);
                });
            },
        );
    }
    group.finish();
}

pub fn bench_strong_checksum_non_overlapping(c: &mut Criterion) {
    let mut group = c.benchmark_group("strong_checksum_non_overlapping");
    for length in (0..=MAX_SIZE).step_by(STEP_SIZE) {
        group.throughput(Throughput::Bytes(length as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(length),
            &length,
            |b, &length| {
                let mut buffer = vec![0u8; length];
                let mut rng = thread_rng();

                b.iter(|| {
                    rng.fill_bytes(&mut buffer);
                    StrongCheckSum::new()
                        .checksums_non_overlapping(&buffer)
                        .for_each(drop);
                });
            },
        );
    }
    group.finish();
}

pub fn bench_weak_checksum_rolling(c: &mut Criterion) {
    let mut group = c.benchmark_group("weak_checksum_rolling");
    for length in (0..=MAX_SIZE).step_by(STEP_SIZE) {
        group.throughput(Throughput::Bytes(length as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(length),
            &length,
            |b, &length| {
                let mut buffer = vec![0u8; length];
                let mut rng = thread_rng();

                b.iter(|| {
                    rng.fill_bytes(&mut buffer);
                    WeakCheckSum::new()
                        .checksums(&buffer)
                        .for_each(drop);
                });
            },
        );
    }
    group.finish();
}

pub fn bench_weak_checksum_non_overlapping(c: &mut Criterion) {
    let mut group = c.benchmark_group("weak_checksum_non_overlapping");
    for length in (0..=MAX_SIZE).step_by(STEP_SIZE) {
        group.throughput(Throughput::Bytes(length as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(length),
            &length,
            |b, &length| {
                let mut buffer = vec![0u8; length];
                let mut rng = thread_rng();

                b.iter(|| {
                    rng.fill_bytes(&mut buffer);
                    WeakCheckSum::new()
                        .checksums_non_overlapping(&buffer)
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
    targets = 
        bench_weak_checksum_rolling,
        bench_weak_checksum_non_overlapping, 
        bench_strong_checksum_rolling, 
        bench_strong_checksum_non_overlapping, 
}
