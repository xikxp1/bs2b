use std::f32::consts::PI;

use bs2b::Bs2b;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

fn generate_interleaved(frames: usize, sample_rate: f32) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    for frame in 0..frames {
        let t = frame as f32 / sample_rate;
        out[frame * 2] = (2.0 * PI * 440.0 * t).sin() * 0.8;
        out[frame * 2 + 1] = (2.0 * PI * 554.37 * t).sin() * 0.8;
    }
    out
}

fn bench_interleaved(c: &mut Criterion) {
    let frames = 48_000;
    let input = generate_interleaved(frames, 48_000.0);

    let mut group = c.benchmark_group("bs2b_interleaved");
    group.throughput(Throughput::Elements((frames * 2) as u64));
    group.bench_function("f32_48k_1s", |b| {
        b.iter_batched(
            || {
                let mut dsp = Bs2b::default();
                dsp.set_sample_rate(48_000)
                    .expect("48kHz sample rate should be valid");
                (dsp, input.clone())
            },
            |(mut dsp, mut data)| {
                dsp.process_interleaved(&mut data)
                    .expect("input data has even stereo sample count");
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_planar(c: &mut Criterion) {
    let frames = 48_000;
    let interleaved = generate_interleaved(frames, 48_000.0);
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    for frame in interleaved.chunks_exact(2) {
        left.push(frame[0]);
        right.push(frame[1]);
    }

    let mut group = c.benchmark_group("bs2b_planar");
    group.throughput(Throughput::Elements((frames * 2) as u64));
    group.bench_function("f32_48k_1s", |b| {
        b.iter_batched(
            || {
                let mut dsp = Bs2b::default();
                dsp.set_sample_rate(48_000)
                    .expect("48kHz sample rate should be valid");
                (dsp, left.clone(), right.clone())
            },
            |(mut dsp, mut left, mut right)| {
                dsp.process_planar(&mut left, &mut right)
                    .expect("planar buffers have matching lengths");
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(benches, bench_interleaved, bench_planar);
criterion_main!(benches);
