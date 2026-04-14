use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde::Serialize;
use textquest_common::ipc::Command;

/// Helper to serialize Commands using serde_json (for benchmarking purposes).
/// This measures serialization overhead for IPC message preparation.
fn bench_command_serialization<T: Serialize>(c: &mut Criterion, name: &str, cmd: T) {
    let mut group = c.benchmark_group("command_serialization");
    group.bench_function(name, |b| {
        b.iter(|| {
            let serialized = serde_json::to_vec(black_box(&cmd)).expect("serialization failed");
            serialized.len()
        });
    });
    group.finish();
}

/// Benchmark Ping command (minimal payload).
fn bench_ping_command(c: &mut Criterion) {
    bench_command_serialization(c, "command_ping_serde_json", Command::Ping);
}

/// Benchmark MoveTo command (complex payload with floats).
fn bench_moveto_command(c: &mut Criterion) {
    bench_command_serialization(
        c,
        "command_moveto_serde_json",
        Command::MoveTo {
            x: 100.5,
            y: 200.5,
            z: 50.0,
        },
    );
}

/// Benchmark StopMovement command.
fn bench_stop_movement_command(c: &mut Criterion) {
    bench_command_serialization(c, "command_stop_movement_serde_json", Command::StopMovement);
}

/// Benchmark CastSpell command with optional target.
fn bench_cast_spell_command(c: &mut Criterion) {
    bench_command_serialization(
        c,
        "command_cast_spell_serde_json",
        Command::CastSpell {
            spell_slot: 3,
            target_id: Some(1234),
            kill: false,
            recast: 0,
        },
    );
}

/// Benchmark SetTarget command.
fn bench_set_target_command(c: &mut Criterion) {
    bench_command_serialization(
        c,
        "command_set_target_serde_json",
        Command::SetTarget { spawn_id: 5678 },
    );
}

/// Benchmark SlashCommand with string payload.
fn bench_slash_command(c: &mut Criterion) {
    bench_command_serialization(
        c,
        "command_slash_command_serde_json",
        Command::SlashCommand {
            command: "/target Mob Name".to_string(),
        },
    );
}

criterion_group!(
    benches,
    bench_ping_command,
    bench_moveto_command,
    bench_stop_movement_command,
    bench_cast_spell_command,
    bench_set_target_command,
    bench_slash_command
);
criterion_main!(benches);
