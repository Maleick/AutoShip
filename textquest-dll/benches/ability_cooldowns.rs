use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};

#[allow(dead_code, unused_imports)]
#[path = "../src/combat/ability_cooldowns.rs"]
mod ability_cooldowns;

use ability_cooldowns::{AbilityCooldownTracker, SharedCooldownKey};

fn bench_shared_timer_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("ability_cooldowns");
    group.throughput(Throughput::Elements(1));

    group.bench_function("can_use_string_shared_key", |b| {
        let mut tracker = AbilityCooldownTracker::new();
        tracker.consume(
            black_box(1001),
            Some(black_box(600)),
            Some(black_box("warrior-offense")),
            Some(black_box(600)),
            black_box(0),
        );

        b.iter(|| {
            black_box(tracker.can_use(
                black_box(1002),
                Some(black_box("warrior-offense")),
                black_box(1),
            ));
        });
    });

    group.bench_function("can_use_precomputed_shared_key", |b| {
        let shared_key = SharedCooldownKey::from_name(black_box("warrior-offense"));
        let mut tracker = AbilityCooldownTracker::new();
        tracker.consume_with_shared_key(
            black_box(1001),
            Some(black_box(600)),
            Some(black_box(shared_key)),
            Some(black_box(600)),
            black_box(0),
        );

        b.iter(|| {
            black_box(tracker.can_use_with_shared_key(
                black_box(1002),
                Some(black_box(shared_key)),
                black_box(1),
            ));
        });
    });

    group.bench_function("tick_64_active_cooldowns", |b| {
        b.iter_batched(
            || {
                let mut tracker = AbilityCooldownTracker::new();
                for ability_id in 0..64 {
                    tracker.consume(black_box(ability_id), Some(black_box(600)), None, None, 0);
                }
                tracker
            },
            |mut tracker| {
                tracker.tick(black_box(1));
                black_box(tracker);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_shared_timer_checks);
criterion_main!(benches);
