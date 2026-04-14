use criterion::{Criterion, black_box, criterion_group, criterion_main};

/// Simulated benchmark for SpawnInfo field-by-field reads.
///
/// In the actual implementation, SpawnInfo is read field-by-field via `proc.read::<T>(addr + OFFSET)`.
/// This benchmark simulates the overhead of multiple sequential reads with type conversions.
fn bench_spawninfo_field_reads(c: &mut Criterion) {
    c.bench_function("spawninfo_field_reads_simulation", |b| {
        b.iter(|| {
            // Simulate reading ~20 fields from SpawnInfo in sequence.
            // Each field represents a separate proc.read::<T>() call.
            let mut values = Vec::new();

            // Simulate reading u32 fields (offsets, level, etc.)
            for i in 0..10 {
                let val: u32 = black_box(i);
                values.push(val);
            }

            // Simulate reading String/Name fields (would be individual char array reads).
            for _ in 0..3 {
                let s = String::from("test_name");
                values.push(s.len() as u32);
            }

            // Simulate reading f32 fields (position coordinates).
            let coords = [(1.0f32, 2.0f32, 3.0f32), (4.0f32, 5.0f32, 6.0f32)];
            for coord in coords.iter() {
                let _ = black_box((coord.0, coord.1, coord.2));
            }

            values
        });
    });
}

/// Benchmark multiple sequential field reads (realistic scenario).
fn bench_spawninfo_batch_reads(c: &mut Criterion) {
    c.bench_function("spawninfo_batch_field_reads", |b| {
        b.iter(|| {
            // Simulate a full spawn read cycle with 30+ field reads.
            let mut result = vec![
                black_box(1234u32),
                black_box(5678u32),
                black_box(9u32),
                black_box(7u32),
            ];

            // Levels and stats
            for i in 0..5 {
                result.push(black_box(i as u8 as u32));
            }

            // Coordinates (3 reads per position)
            for _ in 0..3 {
                result.push(black_box(100.0f32.to_bits()));
            }

            // Heading, movement flags, type
            for i in 0..5 {
                result.push(black_box(i as u32));
            }

            // Status, light, race
            for i in 0..4 {
                result.push(black_box(i as u32));
            }

            result
        });
    });
}

criterion_group!(
    benches,
    bench_spawninfo_field_reads,
    bench_spawninfo_batch_reads
);
criterion_main!(benches);
