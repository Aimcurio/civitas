use std::fs::File;
use std::io::Write;
use std::time::Instant;

use civitas_core::persistence::{load_from_file, save_to_file};
use civitas_core::replay::compute_authoritative_state_hash;
use civitas_core::sim::Simulation;

#[test]
fn measure_hotpaths() {
    let should_record = std::env::var("CIVITAS_RECORD_HOTPATHS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let (mut log, temp_log_path) = if should_record {
        let f = File::create("../../evidence/hotpath_measurements.txt")
            .or_else(|_| File::create("evidence/hotpath_measurements.txt"))
            .expect("Failed to create evidence log");
        (f, None)
    } else {
        let path = std::env::temp_dir().join(format!(
            "civitas_hotpath_{}_{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let f = File::create(&path).expect("Failed to create temporary benchmark log");
        (f, Some(path))
    };

    writeln!(log, "=== CIVITAS-1M HOT PATH TIMING MEASUREMENTS ===").unwrap();
    writeln!(log, "Timestamp: {:?}", Instant::now()).unwrap();

    let scales = [1_000, 10_000, 100_000];

    for &n in &scales {
        writeln!(log, "\n--- Population Scale: N = {} ---", n).unwrap();
        let mut sim = Simulation::new(42, 128, 128, 8, n);

        // 1. Measure step() total
        let start = Instant::now();
        sim.step();
        let step_dur = start.elapsed();
        writeln!(
            log,
            "sim.step() [1 tick]: {:.4} ms",
            step_dur.as_secs_f64() * 1000.0
        )
        .unwrap();

        // 2. Measure compute_authoritative_state_hash
        let start = Instant::now();
        let hash = compute_authoritative_state_hash(&mut sim.world);
        let hash_dur = start.elapsed();
        writeln!(
            log,
            "compute_authoritative_state_hash(): {:.4} ms (hash: {:016x})",
            hash_dur.as_secs_f64() * 1000.0,
            hash
        )
        .unwrap();

        // 3. Measure snapshot persistence save & load
        let temp_save = std::env::temp_dir().join(format!(
            "bench_save_{}_{}_{}.civ",
            std::process::id(),
            n,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let start = Instant::now();
        save_to_file(&mut sim.world, 42, &temp_save).unwrap();
        let save_dur = start.elapsed();
        writeln!(
            log,
            "save_to_file(): {:.4} ms",
            save_dur.as_secs_f64() * 1000.0
        )
        .unwrap();

        let start = Instant::now();
        let _snap = load_from_file(&temp_save).unwrap();
        let load_dur = start.elapsed();
        writeln!(
            log,
            "load_from_file(): {:.4} ms",
            load_dur.as_secs_f64() * 1000.0
        )
        .unwrap();

        let _ = std::fs::remove_file(temp_save);
    }

    if let Some(path) = temp_log_path {
        let _ = std::fs::remove_file(path);
    }
}
