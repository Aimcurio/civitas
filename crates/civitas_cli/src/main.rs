use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

use civitas_core::invariants::verify_invariants;
use civitas_core::persistence::{load_from_file, save_to_file};
use civitas_core::sim::Simulation;

#[cfg(target_os = "windows")]
fn get_process_memory() -> (u64, u64) {
    #[repr(C)]
    #[allow(non_snake_case)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    unsafe {
        let mut counters: ProcessMemoryCounters = std::mem::zeroed();
        counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        let handle = GetCurrentProcess();
        if K32GetProcessMemoryInfo(handle, &mut counters, counters.cb) != 0 {
            (
                counters.working_set_size as u64,
                counters.peak_working_set_size as u64,
            )
        } else {
            (0, 0)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_process_memory() -> (u64, u64) {
    (0, 0)
}

#[derive(Parser, Debug)]
#[command(
    name = "civitas",
    author = "Civitas-1M Authors",
    version = "0.1.0",
    about = "CIVITAS-1M: Frontier-Scale Rust Civilization Simulation"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a deterministic procedural world map and output settlement summary
    Generate {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 128)]
        width: u32,
        #[arg(long, default_value_t = 128)]
        height: u32,
        #[arg(long, default_value_t = 8)]
        settlements: usize,
        #[arg(long, default_value_t = 0)]
        population: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Run headless simulation forward through time
    Run {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 10_000)]
        population: usize,
        #[arg(long, default_value_t = 100)]
        ticks: u64,
        #[arg(long)]
        save_out: Option<PathBuf>,
    },
    /// Run high-performance benchmark suite for scale verification (10K, 100K, 1M)
    Benchmark {
        #[arg(long, default_value_t = 10_000)]
        population: usize,
        #[arg(long, default_value_t = 50)]
        ticks: u64,
        #[arg(long, default_value_t = 5)]
        warmup: u64,
        #[arg(long, default_value_t = 42)]
        seed: u64,
    },
    /// Replay simulation and verify deterministic checksum checkpoints
    Replay {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 10_000)]
        population: usize,
        #[arg(long, default_value_t = 100)]
        ticks: u64,
    },
    /// Inspect and validate save file integrity and schema
    ValidateSave {
        #[arg(long, alias = "input")]
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            seed,
            width,
            height,
            settlements,
            population,
            output,
        } => {
            if width < 16 || height < 16 || width > 2048 || height > 2048 {
                eprintln!("Error: Map dimensions must be between 16x16 and 2048x2048.");
                std::process::exit(1);
            }
            if settlements == 0 || settlements > 256 {
                eprintln!("Error: Settlement count must be between 1 and 256.");
                std::process::exit(1);
            }
            if population > 2_000_000 {
                eprintln!(
                    "Error: Population {} exceeds maximum supported frontier of 2,000,000.",
                    population
                );
                std::process::exit(1);
            }

            println!("=== CIVITAS-1M World Generation ===");
            println!(
                "Seed: {}, Dimensions: {}x{}, Settlement Nodes: {}, Population: {}",
                seed, width, height, settlements, population
            );
            let start = Instant::now();
            let mut sim = Simulation::new(seed, width, height, settlements, population);
            let elapsed = start.elapsed();
            println!(
                "World generated in {:.3} ms",
                elapsed.as_secs_f64() * 1000.0
            );
            let world_map = sim.world.resource::<civitas_core::world::WorldMap>();
            println!(
                "Created {} settlements across terrain.",
                world_map.settlement_positions.len()
            );
            for (sid, pos) in &world_map.settlement_positions {
                println!("  Node #{}: ({}, {})", sid.0, pos.0, pos.1);
            }
            if let Some(path) = output {
                println!("Saving generated world snapshot to {:?}", path);
                match save_to_file(&mut sim.world, seed, &path) {
                    Ok(_) => println!("Save completed successfully with CRC32 integrity check."),
                    Err(e) => eprintln!("Save failed: {}", e),
                }
            }
        }
        Commands::Run {
            seed,
            population,
            ticks,
            save_out,
        } => {
            if population == 0 || population > 2_000_000 {
                eprintln!(
                    "Error: Population must be between 1 and 2,000,000 (requested: {}).",
                    population
                );
                std::process::exit(1);
            }
            if ticks == 0 {
                eprintln!("Error: Simulation ticks must be at least 1.");
                std::process::exit(1);
            }

            println!("=== CIVITAS-1M Simulation Run ===");
            println!("Seed: {}, Citizens: {}, Ticks: {}", seed, population, ticks);
            let init_start = Instant::now();
            let mut sim = Simulation::new(seed, 128, 128, 8, population);
            println!(
                "World & population spawned in {:.3} s",
                init_start.elapsed().as_secs_f64()
            );

            let run_start = Instant::now();
            for t in 1..=ticks {
                sim.step();
                if t % 50 == 0 || t == ticks {
                    let living = sim.living_citizens_count();
                    println!(
                        "Tick {:4} | Living: {:7} | State Hash: {:016x}",
                        t,
                        living,
                        sim.state_hash()
                    );
                }
            }
            let run_elapsed = run_start.elapsed();
            let tps = (ticks as f64) / run_elapsed.as_secs_f64();
            println!(
                "Run completed in {:.3} s (Avg TPS: {:.1})",
                run_elapsed.as_secs_f64(),
                tps
            );

            // Invariant verification
            match sim.check_invariants() {
                Ok(_) => println!("Invariant check: PASSED (All simulation invariants held)"),
                Err(errs) => {
                    println!("Invariant check: FAILED with {} violations!", errs.len());
                    for e in errs.iter().take(5) {
                        println!("  - {}", e);
                    }
                }
            }

            if let Some(path) = save_out {
                println!("Saving snapshot to {:?}", path);
                match save_to_file(&mut sim.world, seed, &path) {
                    Ok(_) => println!("Save completed successfully with CRC32 integrity check."),
                    Err(e) => eprintln!("Save failed: {}", e),
                }
            }
        }
        Commands::Benchmark {
            population,
            ticks,
            warmup,
            seed,
        } => {
            if population == 0 || population > 2_000_000 {
                eprintln!(
                    "Error: Benchmark population must be between 1 and 2,000,000 (requested: {}).",
                    population
                );
                std::process::exit(1);
            }
            if ticks == 0 {
                eprintln!("Error: Benchmark ticks must be at least 1.");
                std::process::exit(1);
            }

            println!("============================================================");
            println!("             CIVITAS-1M PERFORMANCE BENCHMARK               ");
            println!("============================================================");
            println!("OS: {}", std::env::consts::OS);
            println!("Arch: {}", std::env::consts::ARCH);
            println!("Target Population: {} persistent citizens", population);
            println!("Warmup Ticks:      {}", warmup);
            println!("Target Ticks:      {} daily ticks", ticks);
            println!("Random Seed:       {}", seed);
            println!("------------------------------------------------------------");

            println!("Allocating ECS component tables and spawning entities...");
            let init_start = Instant::now();
            let mut sim = Simulation::new(seed, 256, 256, 16, population);
            let init_duration = init_start.elapsed();
            let (init_ws, _) = get_process_memory();
            println!(
                "Spawned {} citizens across 16 settlements in {:.3} s (Initial WS: {:.1} MB)",
                sim.total_citizens_count(),
                init_duration.as_secs_f64(),
                init_ws as f64 / (1024.0 * 1024.0)
            );

            // Warmup ticks
            if warmup > 0 {
                print!("Running {} warmup ticks...", warmup);
                let warmup_start = Instant::now();
                for _ in 0..warmup {
                    sim.step();
                }
                let warmup_duration = warmup_start.elapsed();
                let warmup_tps = (warmup as f64) / warmup_duration.as_secs_f64();
                println!(
                    " done in {:.3} s ({:.1} TPS)",
                    warmup_duration.as_secs_f64(),
                    warmup_tps
                );
            }

            let (post_warmup_ws, _) = get_process_memory();

            println!("Running authoritative multirate simulation benchmark...");
            let mut tick_durations_ms = Vec::with_capacity(ticks as usize);

            let bench_start = Instant::now();
            for _ in 0..ticks {
                let t_start = Instant::now();
                sim.step();
                tick_durations_ms.push(t_start.elapsed().as_secs_f64() * 1000.0);
            }
            let bench_duration = bench_start.elapsed();

            let (final_ws, peak_ws) = get_process_memory();
            let total_secs = bench_duration.as_secs_f64();
            let tps = (ticks as f64) / total_secs;
            let agent_steps = (population as f64) * (ticks as f64);
            let agents_per_sec = agent_steps / total_secs;

            // Compute statistics
            let mut sorted_durations = tick_durations_ms.clone();
            sorted_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let min_ms = sorted_durations.first().copied().unwrap_or(0.0);
            let max_ms = sorted_durations.last().copied().unwrap_or(0.0);
            let p50_idx = ((sorted_durations.len() as f64) * 0.50) as usize;
            let p95_idx = ((sorted_durations.len() as f64) * 0.95) as usize;
            let p99_idx = ((sorted_durations.len() as f64) * 0.99) as usize;

            let p50_ms = sorted_durations[p50_idx.min(sorted_durations.len() - 1)];
            let p95_ms = sorted_durations[p95_idx.min(sorted_durations.len() - 1)];
            let p99_ms = sorted_durations[p99_idx.min(sorted_durations.len() - 1)];
            let mean_ms = tick_durations_ms.iter().sum::<f64>() / (tick_durations_ms.len() as f64);

            let variance = tick_durations_ms
                .iter()
                .map(|&d| (d - mean_ms).powi(2))
                .sum::<f64>()
                / (tick_durations_ms.len() as f64);
            let std_dev_ms = variance.sqrt();

            let to_mb = |bytes: u64| (bytes as f64) / (1024.0 * 1024.0);
            let memory_growth_mb = to_mb(final_ws.saturating_sub(post_warmup_ws));

            println!("------------------------------------------------------------");
            println!("BENCHMARK RESULTS:");
            println!("  Total Ticks Run:        {}", ticks);
            println!("  Total Execution Time:   {:.3} s", total_secs);
            println!("  Overall Average TPS:    {:.2} ticks/sec", tps);
            println!(
                "  Peak Instantaneous TPS: {:.2} ticks/sec",
                if min_ms > 0.0 { 1000.0 / min_ms } else { 0.0 }
            );
            println!(
                "  Throughput:             {:.2} million agent-ticks/sec",
                agents_per_sec / 1_000_000.0
            );
            println!("------------------------------------------------------------");
            println!("TICK LATENCY DISTRIBUTION (ms):");
            println!("  Mean Tick Duration:     {:.2} ms", mean_ms);
            println!("  Std Deviation:          {:.2} ms", std_dev_ms);
            println!("  Min Tick Duration:      {:.2} ms", min_ms);
            println!("  p50 Tick Duration:      {:.2} ms", p50_ms);
            println!("  p95 Tick Duration:      {:.2} ms", p95_ms);
            println!("  p99 Tick Duration:      {:.2} ms", p99_ms);
            println!("  Max Tick Duration:      {:.2} ms", max_ms);
            println!("------------------------------------------------------------");
            println!("MEMORY TELEMETRY (Windows Working Set):");
            println!("  Initial Post-Spawn WS:  {:.1} MB", to_mb(init_ws));
            println!("  Post-Warmup WS:         {:.1} MB", to_mb(post_warmup_ws));
            println!("  Final Benchmark WS:     {:.1} MB", to_mb(final_ws));
            println!("  Peak Working Set:       {:.1} MB", to_mb(peak_ws));
            println!("  Sustained Memory Growth:{:+.2} MB", memory_growth_mb);
            println!("------------------------------------------------------------");
            println!("SIMULATION INTEGRITY:");
            println!("  Living Population:      {}", sim.living_citizens_count());
            println!("  Final State Hash:       {:016x}", sim.state_hash());

            let inv_res = sim.check_invariants();
            assert!(
                inv_res.is_ok(),
                "Post-benchmark invariant check failed: {:?}",
                inv_res.err()
            );
            println!("  Invariants Verification: 100% PASSED");
            println!("============================================================");
        }
        Commands::Replay {
            seed,
            population,
            ticks,
        } => {
            if population == 0 || population > 2_000_000 {
                eprintln!(
                    "Error: Replay population must be between 1 and 2,000,000 (requested: {}).",
                    population
                );
                std::process::exit(1);
            }
            if ticks == 0 {
                eprintln!("Error: Replay ticks must be at least 1.");
                std::process::exit(1);
            }
            println!("=== CIVITAS-1M Deterministic Replay Check ===");
            println!("Phase 1: Generating baseline authoritative trace...");
            let mut sim1 = Simulation::new(seed, 128, 128, 8, population);
            let mut checkpoints = std::collections::BTreeMap::new();
            for t in 1..=ticks {
                sim1.step();
                if t % 20 == 0 || t == ticks {
                    checkpoints.insert(t, sim1.state_hash());
                }
            }

            println!(
                "Phase 2: Executing deterministic replay from seed {}...",
                seed
            );
            let mut sim2 = Simulation::new(seed, 128, 128, 8, population);
            let mut diverged = false;
            for t in 1..=ticks {
                sim2.step();
                if let Some(&expected) = checkpoints.get(&t) {
                    let actual = sim2.state_hash();
                    if actual != expected {
                        println!(
                            "DIVERGENCE at tick {}! Expected {:016x}, got {:016x}",
                            t, expected, actual
                        );
                        diverged = true;
                        break;
                    }
                }
            }

            if !diverged {
                println!(
                    "REPLAY SUCCESSFUL: All {} checkpoints matched bit-for-bit.",
                    checkpoints.len()
                );
            } else {
                std::process::exit(1);
            }
        }
        Commands::ValidateSave { file } => {
            println!("=== CIVITAS-1M Save Validation ===");
            println!("Reading file: {:?}", file);
            match load_from_file(&file) {
                Ok(snapshot) => {
                    println!("CRC32 Integrity Check: PASSED");
                    println!("Simulation Tick:       {}", snapshot.clock.tick);
                    println!("Citizen Entities:      {}", snapshot.citizens.len());
                    println!(
                        "Settlement Count:      {}",
                        snapshot.settlements.settlements.len()
                    );
                    println!(
                        "Household Count:       {}",
                        snapshot.households.households.len()
                    );
                    println!("Recent Events:         {}", snapshot.events.events.len());

                    let mut sim = Simulation::from_snapshot(snapshot);
                    match verify_invariants(&mut sim.world) {
                        Ok(_) => println!("Invariant Check:       PASSED"),
                        Err(e) => println!("Invariant Check:       FAILED ({} errors)", e.len()),
                    }
                }
                Err(e) => {
                    eprintln!("Save validation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
