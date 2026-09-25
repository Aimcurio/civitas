use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

use civitas_core::invariants::verify_invariants;
use civitas_core::persistence::{load_from_file, save_to_file};
use civitas_core::sim::Simulation;

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
        #[arg(long)]
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
        } => {
            println!("=== CIVITAS-1M World Generation ===");
            println!(
                "Seed: {}, Dimensions: {}x{}, Settlement Nodes: {}",
                seed, width, height, settlements
            );
            let start = Instant::now();
            let sim = Simulation::new(seed, width, height, settlements, 0);
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
        }
        Commands::Run {
            seed,
            population,
            ticks,
            save_out,
        } => {
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
            seed,
        } => {
            println!("============================================================");
            println!("             CIVITAS-1M PERFORMANCE BENCHMARK               ");
            println!("============================================================");
            println!("OS: {}", std::env::consts::OS);
            println!("Arch: {}", std::env::consts::ARCH);
            println!("Target Population: {} persistent citizens", population);
            println!("Target Ticks:      {} daily ticks", ticks);
            println!("Random Seed:       {}", seed);
            println!("------------------------------------------------------------");

            println!("Allocating ECS component tables and spawning entities...");
            let init_start = Instant::now();
            let mut sim = Simulation::new(seed, 256, 256, 16, population);
            let init_duration = init_start.elapsed();
            println!(
                "Spawned {} citizens across 16 settlements in {:.3} s",
                sim.total_citizens_count(),
                init_duration.as_secs_f64()
            );

            // Warmup tick
            sim.step();

            println!("Running authoritative multirate simulation benchmark...");
            let bench_start = Instant::now();
            for _ in 0..ticks {
                sim.step();
            }
            let bench_duration = bench_start.elapsed();

            let total_secs = bench_duration.as_secs_f64();
            let tps = (ticks as f64) / total_secs;
            let agent_steps = (population as f64) * (ticks as f64);
            let agents_per_sec = agent_steps / total_secs;

            println!("------------------------------------------------------------");
            println!("BENCHMARK RESULTS:");
            println!("  Total Ticks Run:        {}", ticks);
            println!("  Total Execution Time:   {:.3} s", total_secs);
            println!("  Simulation TPS:         {:.2} ticks/sec", tps);
            println!(
                "  Throughput:             {:.2} million agent-ticks/sec",
                agents_per_sec / 1_000_000.0
            );
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
