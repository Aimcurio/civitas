use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use civitas_core::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, Kinship, MobilityProfile,
    OccupationProfile, PersonalFinances, PhysicalNeeds, SettlementRef,
};
use civitas_core::events::{EventRing, SimEvent};
use civitas_core::household::HouseholdDirectory;
use civitas_core::invariants::verify_invariants;
use civitas_core::persistence::{load_from_file, save_to_file};
use civitas_core::settlement::{SettlementDirectory, SettlementPolicy};
use civitas_core::sim::Simulation;
use civitas_core::types::{CitizenId, MigrationStatus, SettlementId, SimClock};
use civitas_core::world::WorldMap;

#[derive(Debug)]
pub enum SimCommand {
    Play,
    Pause,
    Step(u64),
    SetSpeed(u32),
    Save(PathBuf),
    Load(PathBuf),
    GenerateWorld {
        seed: u64,
        population: usize,
    },
    VerifyInvariants,
    RequestCitizenDetails(CitizenId),
    SetSettlementPolicy {
        settlement_id: SettlementId,
        policy: SettlementPolicy,
    },
    CommissionHousing {
        settlement_id: SettlementId,
    },
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct CitizenDetailView {
    pub meta: CitizenMeta,
    pub demographics: Demographics,
    pub household_ref: HouseholdRef,
    pub settlement_ref: SettlementRef,
    pub occupation: OccupationProfile,
    pub finances: PersonalFinances,
    pub needs: PhysicalNeeds,
    pub mobility: MobilityProfile,
    pub kinship: Kinship,
    pub causal_audit: CausalAudit,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct InTransitMigrantView {
    pub id: CitizenId,
    pub origin: SettlementId,
    pub destination: SettlementId,
    pub progress_ratio: f32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PresentationSnapshot {
    pub tick: u64,
    pub is_running: bool,
    pub speed: u32,
    pub living_population: usize,
    pub total_population: usize,
    pub state_hash: u64,
    pub measured_tps: f64,
    pub world_map: Arc<WorldMap>,
    pub settlements: Arc<SettlementDirectory>,
    pub households: Arc<HouseholdDirectory>,
    pub recent_events: Vec<SimEvent>,
    pub in_transit_migrants: Vec<InTransitMigrantView>,
    pub active_citizen_count: usize,
    pub total_households_count: usize,
}

pub enum WorkerResponse {
    CitizenDetails(Option<CitizenDetailView>),
    StatusMessage(String),
    VerificationResult(Result<(), Vec<String>>),
}

pub struct WorkerHandle {
    pub cmd_sender: SyncSender<SimCommand>,
    pub resp_receiver: Receiver<WorkerResponse>,
    pub snapshot: Arc<Mutex<PresentationSnapshot>>,
    join_handle: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    pub fn new(seed: u64, initial_population: usize) -> Self {
        let (cmd_tx, cmd_rx) = sync_channel::<SimCommand>(128);
        let (resp_tx, resp_rx) = sync_channel::<WorkerResponse>(128);

        let mut initial_sim = Simulation::new(seed, 128, 128, 8, initial_population);
        let initial_snapshot = build_snapshot(&mut initial_sim, false, 1, 0.0);
        let snapshot = Arc::new(Mutex::new(initial_snapshot));

        let shared_snapshot = Arc::clone(&snapshot);

        let join_handle = thread::Builder::new()
            .name("civitas-sim-worker".to_string())
            .spawn(move || {
                run_worker_loop(initial_sim, cmd_rx, resp_tx, shared_snapshot);
            })
            .expect("Failed to spawn simulation worker thread");

        Self {
            cmd_sender: cmd_tx,
            resp_receiver: resp_rx,
            snapshot,
            join_handle: Some(join_handle),
        }
    }

    pub fn send_command(&self, cmd: SimCommand) -> bool {
        self.cmd_sender.send(cmd).is_ok()
    }

    pub fn try_recv_response(&self) -> Option<WorkerResponse> {
        self.resp_receiver.try_recv().ok()
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        let _ = self.cmd_sender.send(SimCommand::Shutdown);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

fn build_snapshot(
    sim: &mut Simulation,
    is_running: bool,
    speed: u32,
    measured_tps: f64,
) -> PresentationSnapshot {
    let tick = sim.world.resource::<SimClock>().tick;
    let world_map = Arc::new(sim.world.resource::<WorldMap>().clone());
    let settlements = Arc::new(sim.world.resource::<SettlementDirectory>().clone());
    let households = Arc::new(sim.world.resource::<HouseholdDirectory>().clone());
    let recent_events = sim
        .world
        .resource::<EventRing>()
        .recent(100)
        .into_iter()
        .cloned()
        .collect();

    let mut living = 0;
    let mut total = 0;
    let mut in_transit = Vec::new();

    let mut query = sim.world.query::<(&CitizenMeta, &MobilityProfile)>();
    for (meta, mobility) in query.iter(&sim.world) {
        total += 1;
        if meta.alive {
            living += 1;
            if let MigrationStatus::InTransit {
                origin,
                destination,
                ticks_remaining,
                total_ticks,
            } = mobility.status
            {
                let progress = if total_ticks > 0 {
                    1.0 - (ticks_remaining as f32 / total_ticks as f32)
                } else {
                    0.5
                };
                in_transit.push(InTransitMigrantView {
                    id: meta.id,
                    origin,
                    destination,
                    progress_ratio: progress.clamp(0.0, 1.0),
                });
            }
        }
    }

    let total_households = households.households.len();

    PresentationSnapshot {
        tick,
        is_running,
        speed,
        living_population: living,
        total_population: total,
        state_hash: 0,
        measured_tps,
        world_map,
        settlements,
        households,
        recent_events,
        in_transit_migrants: in_transit,
        active_citizen_count: living,
        total_households_count: total_households,
    }
}

fn query_citizen_details(sim: &Simulation, cid: CitizenId) -> Option<CitizenDetailView> {
    let entity = sim.get_citizen_entity(cid)?;
    let meta = sim.world.get::<CitizenMeta>(entity)?.clone();
    let demographics = sim.world.get::<Demographics>(entity)?.clone();
    let household_ref = sim.world.get::<HouseholdRef>(entity)?.clone();
    let settlement_ref = sim.world.get::<SettlementRef>(entity)?.clone();
    let occupation = sim.world.get::<OccupationProfile>(entity)?.clone();
    let finances = sim.world.get::<PersonalFinances>(entity)?.clone();
    let needs = sim.world.get::<PhysicalNeeds>(entity)?.clone();
    let mobility = sim.world.get::<MobilityProfile>(entity)?.clone();
    let kinship = sim.world.get::<Kinship>(entity)?.clone();
    let causal_audit = sim.world.get::<CausalAudit>(entity)?.clone();

    Some(CitizenDetailView {
        meta,
        demographics,
        household_ref,
        settlement_ref,
        occupation,
        finances,
        needs,
        mobility,
        kinship,
        causal_audit,
    })
}

fn run_worker_loop(
    mut sim: Simulation,
    cmd_rx: Receiver<SimCommand>,
    resp_tx: SyncSender<WorkerResponse>,
    shared_snapshot: Arc<Mutex<PresentationSnapshot>>,
) {
    let mut is_running = false;
    let mut speed: u32 = 1;
    let mut last_snapshot_time = Instant::now();
    let mut tps_counter = 0u32;
    let mut tps_window_start = Instant::now();
    let mut measured_tps = 0.0;

    'worker: loop {
        // Drain pending commands
        loop {
            match cmd_rx.try_recv() {
                Ok(cmd) => match cmd {
                    SimCommand::Play => {
                        is_running = true;
                        let _ = resp_tx.send(WorkerResponse::StatusMessage(
                            "Simulation running (worker thread)".to_string(),
                        ));
                    }
                    SimCommand::Pause => {
                        is_running = false;
                        let _ = resp_tx.send(WorkerResponse::StatusMessage(
                            "Simulation paused".to_string(),
                        ));
                    }
                    SimCommand::Step(n) => {
                        is_running = false;
                        let start = Instant::now();
                        for _ in 0..n {
                            sim.step();
                            tps_counter += 1;
                        }
                        let elapsed = start.elapsed().as_secs_f64();
                        if elapsed > 0.0 {
                            measured_tps = (n as f64) / elapsed;
                        }
                        let cur_tick = sim.world.resource::<SimClock>().tick;
                        let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                            "Stepped {} ticks. Current tick: {}",
                            n, cur_tick
                        )));
                        // Force snapshot update after step
                        let snap = build_snapshot(&mut sim, is_running, speed, measured_tps);
                        if let Ok(mut lock) = shared_snapshot.lock() {
                            *lock = snap;
                        }
                    }
                    SimCommand::SetSpeed(s) => {
                        speed = s.clamp(1, 100);
                        let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                            "Simulation speed set to {}x",
                            speed
                        )));
                    }
                    SimCommand::Save(path) => {
                        let seed = sim.seed;
                        match save_to_file(
                            &mut sim.world,
                            seed,
                            path.to_str().unwrap_or("save.civ"),
                        ) {
                            Ok(_) => {
                                let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                                    "Saved snapshot to {:?}",
                                    path
                                )));
                            }
                            Err(e) => {
                                let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                                    "Save error: {}",
                                    e
                                )));
                            }
                        }
                    }
                    SimCommand::Load(path) => {
                        match load_from_file(path.to_str().unwrap_or("save.civ")) {
                            Ok(snapshot) => {
                                is_running = false;
                                sim = Simulation::from_snapshot(snapshot);
                                let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                                    "Loaded snapshot from {:?}",
                                    path
                                )));
                                let snap =
                                    build_snapshot(&mut sim, is_running, speed, measured_tps);
                                if let Ok(mut lock) = shared_snapshot.lock() {
                                    *lock = snap;
                                }
                            }
                            Err(e) => {
                                let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                                    "Load error: {}",
                                    e
                                )));
                            }
                        }
                    }
                    SimCommand::GenerateWorld { seed, population } => {
                        is_running = false;
                        sim = Simulation::new(seed, 128, 128, 8, population);
                        let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                            "Generated new world with seed {} (pop: {})",
                            seed, population
                        )));
                        let snap = build_snapshot(&mut sim, is_running, speed, measured_tps);
                        if let Ok(mut lock) = shared_snapshot.lock() {
                            *lock = snap;
                        }
                    }
                    SimCommand::VerifyInvariants => {
                        let res = verify_invariants(&mut sim.world);
                        let _ = resp_tx.send(WorkerResponse::VerificationResult(res));
                    }
                    SimCommand::RequestCitizenDetails(cid) => {
                        let details = query_citizen_details(&sim, cid);
                        let _ = resp_tx.send(WorkerResponse::CitizenDetails(details));
                    }
                    SimCommand::SetSettlementPolicy {
                        settlement_id,
                        policy,
                    } => {
                        if let Some(mut settlements) =
                            sim.world.get_resource_mut::<SettlementDirectory>()
                        {
                            if let Some(s) = settlements.get_mut(settlement_id) {
                                s.set_policy(policy.clone());
                                let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                                    "Settlement #{} policy updated: Tax {:.0}%, Relief {}, Wage {:+.0}%",
                                    settlement_id.0,
                                    policy.tax_rate * 100.0,
                                    if policy.emergency_relief { "ON" } else { "OFF" },
                                    policy.wage_bonus * 100.0
                                )));
                            }
                        }
                        let snap = build_snapshot(&mut sim, is_running, speed, measured_tps);
                        if let Ok(mut lock) = shared_snapshot.lock() {
                            *lock = snap;
                        }
                    }
                    SimCommand::CommissionHousing { settlement_id } => {
                        let res = if let Some(mut settlements) =
                            sim.world.get_resource_mut::<SettlementDirectory>()
                        {
                            if let Some(s) = settlements.get_mut(settlement_id) {
                                s.commission_housing_expansion()
                            } else {
                                Err("Settlement not found")
                            }
                        } else {
                            Err("Settlement directory unavailable")
                        };
                        match res {
                            Ok(()) => {
                                let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                                    "Housing expanded at Settlement #{} (+25 capacity)",
                                    settlement_id.0
                                )));
                            }
                            Err(err) => {
                                let _ = resp_tx.send(WorkerResponse::StatusMessage(format!(
                                    "Cannot expand housing: {}",
                                    err
                                )));
                            }
                        }
                        let snap = build_snapshot(&mut sim, is_running, speed, measured_tps);
                        if let Ok(mut lock) = shared_snapshot.lock() {
                            *lock = snap;
                        }
                    }
                    SimCommand::Shutdown => {
                        break 'worker;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'worker,
            }
        }

        // Run simulation ticks if active
        if is_running {
            for _ in 0..speed {
                sim.step();
                tps_counter += 1;
            }
        }

        // Measure TPS every 500ms
        let now = Instant::now();
        let elapsed_tps = now.duration_since(tps_window_start).as_secs_f64();
        if elapsed_tps >= 0.5 {
            measured_tps = (tps_counter as f64) / elapsed_tps;
            tps_counter = 0;
            tps_window_start = now;
        }

        // Update presentation snapshot at maximum 30Hz or immediately on tick change
        if now.duration_since(last_snapshot_time) >= Duration::from_millis(33) {
            let snap = build_snapshot(&mut sim, is_running, speed, measured_tps);
            if let Ok(mut lock) = shared_snapshot.lock() {
                *lock = snap;
            }
            last_snapshot_time = now;
        }

        // Prevent 100% spin if paused or if fast
        if !is_running {
            thread::sleep(Duration::from_millis(16));
        } else if speed <= 2 {
            thread::sleep(Duration::from_millis(1));
        }
    }
}
