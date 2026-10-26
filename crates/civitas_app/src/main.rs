use civitas_app::worker;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32, Key, Pos2, Rect, Stroke, Vec2};
use egui_plot::{Line, Plot, PlotPoints};

use civitas_core::household::Household;
use civitas_core::settlement::SettlementPolicy;
use civitas_core::types::{
    Biome, CitizenId, HouseholdId, OccupationType, ResourceType, SettlementId,
};
use worker::{CitizenDetailView, PresentationSnapshot, SimCommand, WorkerHandle, WorkerResponse};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ViewTab {
    WorldMap,
    SettlementInspector,
    CitizenInspector,
    HouseholdInspector,
    Analytics,
    EventLog,
}

struct CivitasApp {
    worker: WorkerHandle,
    snapshot: PresentationSnapshot,
    selected_settlement: Option<SettlementId>,
    selected_citizen: Option<CitizenId>,
    inspected_citizen_details: Option<CitizenDetailView>,
    selected_household: Option<HouseholdId>,
    household_search_text: String,
    household_page: usize,
    citizen_search_text: String,
    save_path_text: String,
    status_message: String,
    active_tab: ViewTab,

    // Policy controls
    policy_tax_rate: f32,
    policy_emergency_relief: bool,
    policy_wage_bonus: f32,
    policy_synced_settlement: Option<SettlementId>,

    // Time-series history using VecDeque for O(1) push/pop
    history_ticks: VecDeque<f64>,
    history_living_pop: VecDeque<f64>,
    history_avg_food_price: VecDeque<f64>,
    history_avg_wage: VecDeque<f64>,
    last_analytics_tick: u64,

    // Camera
    camera_offset: Vec2,
    camera_zoom: f32,

    // World creation parameters
    new_seed: u64,
    new_population: usize,
}

#[derive(Clone, Debug)]
pub struct CivitasAppConfig {
    pub seed: u64,
    pub population: usize,
    pub initial_tab: ViewTab,
    pub auto_play: bool,
    pub speed: u32,
    pub initial_citizen: Option<CitizenId>,
    pub initial_settlement: Option<SettlementId>,
    pub initial_household: Option<HouseholdId>,
}

impl Default for CivitasAppConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            population: 5_000,
            initial_tab: ViewTab::WorldMap,
            auto_play: false,
            speed: 1,
            initial_citizen: Some(CitizenId(1)),
            initial_settlement: Some(SettlementId(0)),
            initial_household: Some(HouseholdId(1)),
        }
    }
}

fn parse_args() -> CivitasAppConfig {
    let mut config = CivitasAppConfig::default();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--tab" if i + 1 < args.len() => {
                i += 1;
                config.initial_tab = match args[i].to_lowercase().as_str() {
                    "settlements" | "settlement" => ViewTab::SettlementInspector,
                    "citizens" | "citizen" => ViewTab::CitizenInspector,
                    "households" | "household" => ViewTab::HouseholdInspector,
                    "analytics" | "charts" => ViewTab::Analytics,
                    "events" | "eventlog" | "log" => ViewTab::EventLog,
                    _ => ViewTab::WorldMap,
                };
            }
            "--play" => {
                config.auto_play = true;
            }
            "--speed" if i + 1 < args.len() => {
                i += 1;
                if let Ok(s) = args[i].parse::<u32>() {
                    config.speed = s.clamp(1, 100);
                }
            }
            "--citizen" if i + 1 < args.len() => {
                i += 1;
                if let Ok(id) = args[i].parse::<u64>() {
                    config.initial_citizen = Some(CitizenId(id));
                }
            }
            "--settlement" if i + 1 < args.len() => {
                i += 1;
                if let Ok(id) = args[i].parse::<u16>() {
                    config.initial_settlement = Some(SettlementId(id));
                }
            }
            "--household" if i + 1 < args.len() => {
                i += 1;
                if let Ok(id) = args[i].parse::<u32>() {
                    config.initial_household = Some(HouseholdId(id));
                }
            }
            "--seed" if i + 1 < args.len() => {
                i += 1;
                if let Ok(s) = args[i].parse::<u64>() {
                    config.seed = s;
                }
            }
            "--population" if i + 1 < args.len() => {
                i += 1;
                if let Ok(p) = args[i].parse::<usize>() {
                    config.population = p;
                }
            }
            _ => {}
        }
        i += 1;
    }
    config
}

impl CivitasApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, config: CivitasAppConfig) -> Self {
        let seed = config.seed;
        let population = config.population;
        let worker = WorkerHandle::new(seed, population);
        let initial_snapshot = {
            let lock = worker.snapshot.lock().unwrap();
            lock.clone()
        };

        if config.auto_play {
            worker.send_command(SimCommand::Play);
        }
        if config.speed > 1 {
            worker.send_command(SimCommand::SetSpeed(config.speed));
        }

        let initial_citizen = config.initial_citizen.or(Some(CitizenId(1)));
        let initial_settlement = config.initial_settlement.or(Some(SettlementId(0)));
        let initial_household = config.initial_household.or(Some(HouseholdId(1)));

        let mut app = Self {
            worker,
            snapshot: initial_snapshot,
            selected_settlement: initial_settlement,
            selected_citizen: initial_citizen,
            inspected_citizen_details: None,
            selected_household: initial_household,
            household_search_text: String::new(),
            household_page: 0,
            citizen_search_text: initial_citizen.map_or("1".to_string(), |c| c.0.to_string()),
            save_path_text: "world_save.civ".to_string(),
            status_message: "Ready. Background simulation worker initialized.".to_string(),
            active_tab: config.initial_tab,
            policy_tax_rate: 0.10,
            policy_emergency_relief: true,
            policy_wage_bonus: 0.0,
            policy_synced_settlement: None,
            history_ticks: VecDeque::with_capacity(300),
            history_living_pop: VecDeque::with_capacity(300),
            history_avg_food_price: VecDeque::with_capacity(300),
            history_avg_wage: VecDeque::with_capacity(300),
            last_analytics_tick: 0,
            camera_offset: Vec2::ZERO,
            camera_zoom: 1.0,
            new_seed: 12345,
            new_population: 5_000,
        };

        if let Some(cid) = initial_citizen {
            app.request_citizen_details(cid);
        }
        app
    }

    fn request_citizen_details(&mut self, cid: CitizenId) {
        self.selected_citizen = Some(cid);
        self.worker
            .send_command(SimCommand::RequestCitizenDetails(cid));
    }

    fn record_analytics(&mut self) {
        let tick = self.snapshot.tick;
        if tick == self.last_analytics_tick {
            return;
        }
        self.last_analytics_tick = tick;

        let living = self.snapshot.living_population as f64;
        let s_dir = &self.snapshot.settlements;
        let mut total_food_price = 0.0;
        let mut total_wage = 0.0;
        let count = s_dir.settlements.len().max(1) as f64;

        for s in s_dir.settlements.values() {
            total_food_price += s.get_price(ResourceType::Food) as f64;
            total_wage += s.average_wage() as f64;
        }

        self.history_ticks.push_back(tick as f64);
        self.history_living_pop.push_back(living);
        self.history_avg_food_price
            .push_back(total_food_price / count);
        self.history_avg_wage.push_back(total_wage / count);

        // Cap at 300 points using O(1) pop_front
        if self.history_ticks.len() > 300 {
            self.history_ticks.pop_front();
            self.history_living_pop.pop_front();
            self.history_avg_food_price.pop_front();
            self.history_avg_wage.pop_front();
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Space: Toggle Play / Pause
            if i.key_pressed(Key::Space) {
                if self.snapshot.is_running {
                    self.worker.send_command(SimCommand::Pause);
                } else {
                    self.worker.send_command(SimCommand::Play);
                }
            }

            // Period (.): Step 1 tick
            if i.key_pressed(Key::Period) {
                self.worker.send_command(SimCommand::Step(1));
            }

            // Number keys 1-5 for simulation speeds
            if i.modifiers.is_none() {
                if i.key_pressed(Key::Num1) {
                    self.worker.send_command(SimCommand::SetSpeed(1));
                } else if i.key_pressed(Key::Num2) {
                    self.worker.send_command(SimCommand::SetSpeed(2));
                } else if i.key_pressed(Key::Num3) {
                    self.worker.send_command(SimCommand::SetSpeed(5));
                } else if i.key_pressed(Key::Num4) {
                    self.worker.send_command(SimCommand::SetSpeed(10));
                } else if i.key_pressed(Key::Num5) {
                    self.worker.send_command(SimCommand::SetSpeed(50));
                }
            }

            // Ctrl+1 through Ctrl+6: Switch view tabs
            if i.modifiers.command {
                if i.key_pressed(Key::Num1) {
                    self.active_tab = ViewTab::WorldMap;
                } else if i.key_pressed(Key::Num2) {
                    self.active_tab = ViewTab::SettlementInspector;
                } else if i.key_pressed(Key::Num3) {
                    self.active_tab = ViewTab::CitizenInspector;
                } else if i.key_pressed(Key::Num4) {
                    self.active_tab = ViewTab::HouseholdInspector;
                } else if i.key_pressed(Key::Num5) {
                    self.active_tab = ViewTab::Analytics;
                } else if i.key_pressed(Key::Num6) {
                    self.active_tab = ViewTab::EventLog;
                }
            }
        });
    }

    fn poll_worker_updates(&mut self) {
        // Read latest presentation snapshot without per-frame cloning
        if let Ok(lock) = self.worker.snapshot.try_lock() {
            self.snapshot = lock.clone();
        }

        // Drain any pending responses
        while let Some(resp) = self.worker.try_recv_response() {
            match resp {
                WorkerResponse::CitizenDetails(details) => {
                    self.inspected_citizen_details = details;
                }
                WorkerResponse::StatusMessage(msg) => {
                    self.status_message = msg;
                }
                WorkerResponse::VerificationResult(res) => match res {
                    Ok(_) => {
                        self.status_message = "All invariants 100% verified!".to_string();
                    }
                    Err(errs) => {
                        self.status_message =
                            format!("Invariant violation: {} errors detected!", errs.len());
                    }
                },
            }
        }

        self.record_analytics();
    }
}

impl eframe::App for CivitasApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard_shortcuts(ctx);
        self.poll_worker_updates();

        // Repaint periodically when simulation is running
        if self.snapshot.is_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        // Top Control Panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("CIVITAS-1M");
                ui.separator();

                // Simulation Controls
                if ui
                    .button(if self.snapshot.is_running {
                        "Pause (Space)"
                    } else {
                        "Play (Space)"
                    })
                    .clicked()
                {
                    if self.snapshot.is_running {
                        self.worker.send_command(SimCommand::Pause);
                    } else {
                        self.worker.send_command(SimCommand::Play);
                    }
                }

                if ui.button("Step 1 (.)").clicked() {
                    self.worker.send_command(SimCommand::Step(1));
                }

                ui.separator();
                ui.label("Speed:");
                for &s in &[1, 2, 5, 10, 50] {
                    if ui
                        .selectable_label(self.snapshot.speed == s, format!("{}x", s))
                        .clicked()
                    {
                        self.worker.send_command(SimCommand::SetSpeed(s));
                    }
                }

                ui.separator();
                ui.label(format!("Tick: {}", self.snapshot.tick));
                ui.label(format!(
                    "Population: {}/{}",
                    self.snapshot.living_population, self.snapshot.total_population
                ));
                ui.label(format!("Worker TPS: {:.1}", self.snapshot.measured_tps));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Verify Invariants").clicked() {
                        self.worker.send_command(SimCommand::VerifyInvariants);
                    }
                });
            });

            ui.separator();

            // View Tabs
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::WorldMap,
                    "World Map (Ctrl+1)",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::SettlementInspector,
                    "Settlements (Ctrl+2)",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::CitizenInspector,
                    "Citizen Inspector (Ctrl+3)",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::HouseholdInspector,
                    "Households (Ctrl+4)",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::Analytics,
                    "Analytics & Charts (Ctrl+5)",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::EventLog,
                    "Event Log (Ctrl+6)",
                );
            });
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.monospace(&self.status_message);
            });
        });

        // Left Navigation / World Config Panel
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("World Management");
                ui.separator();

                ui.label("Seed:");
                ui.add(egui::DragValue::new(&mut self.new_seed));

                ui.label("Initial Population:");
                ui.add(egui::Slider::new(&mut self.new_population, 100..=50_000).logarithmic(true));

                if ui.button("Generate New World").clicked() {
                    self.worker.send_command(SimCommand::GenerateWorld {
                        seed: self.new_seed,
                        population: self.new_population,
                    });
                    self.selected_settlement = Some(SettlementId(0));
                    self.selected_citizen = Some(CitizenId(1));
                    self.history_ticks.clear();
                    self.history_living_pop.clear();
                    self.history_avg_food_price.clear();
                    self.history_avg_wage.clear();
                    self.last_analytics_tick = 0;
                }

                ui.separator();
                ui.heading("Save / Load");
                ui.horizontal(|ui| {
                    ui.label("File:");
                    ui.text_edit_singleline(&mut self.save_path_text);
                });

                ui.horizontal(|ui| {
                    if ui.button("Save State").clicked() {
                        let path = PathBuf::from(&self.save_path_text);
                        self.worker.send_command(SimCommand::Save(path));
                    }

                    if ui.button("Load State").clicked() {
                        let path = PathBuf::from(&self.save_path_text);
                        self.worker.send_command(SimCommand::Load(path));
                        self.history_ticks.clear();
                        self.history_living_pop.clear();
                        self.history_avg_food_price.clear();
                        self.history_avg_wage.clear();
                        self.last_analytics_tick = 0;
                    }
                });

                ui.separator();
                ui.heading("Settlements Quick List");
                let s_dir = Arc::clone(&self.snapshot.settlements);
                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .show(ui, |ui| {
                        for (sid, s) in &s_dir.settlements {
                            let is_selected = self.selected_settlement == Some(*sid);
                            let label = format!("{} (Pop: {})", s.name, s.population);
                            if ui.selectable_label(is_selected, label).clicked() {
                                self.selected_settlement = Some(*sid);
                                self.active_tab = ViewTab::SettlementInspector;
                            }
                        }
                    });
            });

        // Central Main View Area
        egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
            ViewTab::WorldMap => self.render_world_map(ui),
            ViewTab::SettlementInspector => self.render_settlement_inspector(ui),
            ViewTab::CitizenInspector => self.render_citizen_inspector(ui),
            ViewTab::HouseholdInspector => self.render_household_inspector(ui),
            ViewTab::Analytics => self.render_analytics(ui),
            ViewTab::EventLog => self.render_event_log(ui),
        });
    }
}

impl CivitasApp {
    fn render_world_map(&mut self, ui: &mut egui::Ui) {
        ui.heading("World Map View");
        ui.label("Pan with drag, zoom with scroll. Click settlement nodes to inspect. In-transit migrants visible along routes.");

        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;

        // Camera handling
        if response.dragged() {
            self.camera_offset += response.drag_delta();
        }
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            self.camera_zoom = (self.camera_zoom * (1.0 + scroll * 0.001)).clamp(0.4, 4.0);
        }

        // Draw background
        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 24, 32));

        let world_map = Arc::clone(&self.snapshot.world_map);
        let settlements = Arc::clone(&self.snapshot.settlements);
        let migrants = &self.snapshot.in_transit_migrants;

        let center = rect.center() + self.camera_offset;
        let scale = 4.0 * self.camera_zoom;

        let map_to_screen = |mx: u32, my: u32| -> Pos2 {
            Pos2::new(
                center.x + ((mx as f32) - (world_map.width as f32) / 2.0) * scale,
                center.y + ((my as f32) - (world_map.height as f32) / 2.0) * scale,
            )
        };

        // Draw low-res terrain
        let step = 4;
        for y in (0..world_map.height).step_by(step) {
            for x in (0..world_map.width).step_by(step) {
                if let Some(tile) = world_map.get_tile(x, y) {
                    let color = match tile.biome {
                        Biome::Water => Color32::from_rgb(30, 70, 140),
                        Biome::Plains => Color32::from_rgb(70, 130, 60),
                        Biome::Forest => Color32::from_rgb(30, 90, 40),
                        Biome::Hills => Color32::from_rgb(140, 120, 80),
                        Biome::Mountains => Color32::from_rgb(180, 180, 190),
                    };

                    let p = map_to_screen(x, y);
                    let tile_rect = Rect::from_min_size(p, Vec2::splat(scale * (step as f32)));
                    if rect.intersects(tile_rect) {
                        painter.rect_filled(tile_rect, 0.0, color);
                    }
                }
            }
        }

        // Draw travel connection routes
        for (a, b) in world_map.settlement_distances.keys() {
            if a.0 < b.0 {
                if let (Some(&(ax, ay)), Some(&(bx, by))) = (
                    world_map.settlement_positions.get(a),
                    world_map.settlement_positions.get(b),
                ) {
                    let pa = map_to_screen(ax, ay);
                    let pb = map_to_screen(bx, by);
                    painter.line_segment(
                        [pa, pb],
                        Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(220, 220, 220, 60)),
                    );
                }
            }
        }

        // CIV-10: Render In-Transit Migrants along routes between settlements
        for migrant in migrants {
            if let (Some(&(ox, oy)), Some(&(dx, dy))) = (
                world_map.settlement_positions.get(&migrant.origin),
                world_map.settlement_positions.get(&migrant.destination),
            ) {
                let po = map_to_screen(ox, oy);
                let pd = map_to_screen(dx, dy);
                let t = migrant.progress_ratio;
                let cur_pos = Pos2::new(po.x + (pd.x - po.x) * t, po.y + (pd.y - po.y) * t);

                // Bright glowing migrant dot
                painter.circle_filled(
                    cur_pos,
                    4.0 * self.camera_zoom.clamp(0.8, 2.0),
                    Color32::from_rgb(255, 220, 50),
                );
                painter.circle_stroke(
                    cur_pos,
                    5.0 * self.camera_zoom.clamp(0.8, 2.0),
                    Stroke::new(1.0_f32, Color32::from_rgb(255, 100, 0)),
                );
            }
        }

        // Draw settlement nodes with Flannery power-law radius, chromatic solvency halo, and interactive HUD
        let mouse_pos = ui.input(|i| i.pointer.hover_pos());
        let mut hovered_settlement: Option<(
            SettlementId,
            civitas_core::settlement::Settlement,
            Pos2,
            f32,
        )> = None;

        for (sid, s) in &settlements.settlements {
            let p = map_to_screen(s.x, s.y);
            let pop = (s.population as f32).max(1.0);
            let radius = (pop.powf(0.57) * 0.45 * self.camera_zoom).clamp(10.0, 52.0);

            let is_selected = self.selected_settlement == Some(*sid);
            let is_hovered = mouse_pos.is_some_and(|m| p.distance(m) <= radius + 6.0);
            if is_hovered {
                hovered_settlement = Some((*sid, s.clone(), p, radius));
            }

            let is_crisis = s.is_crisis();
            let is_warning = s.is_warning();

            let halo_color = if is_crisis {
                Color32::from_rgb(220, 40, 40) // Crimson
            } else if is_warning {
                Color32::from_rgb(235, 145, 30) // Amber
            } else {
                Color32::from_rgb(46, 204, 113) // Emerald
            };

            if is_crisis {
                let time_sec = ui.input(|i| i.time);
                let phase = (time_sec * 2.0).fract() as f32;
                let pulse_r = radius + (phase * 20.0);
                let alpha = ((1.0 - phase) * 160.0) as u8;
                painter.circle_stroke(
                    p,
                    pulse_r,
                    Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(220, 40, 40, alpha)),
                );
            }

            let core_fill = if is_selected {
                Color32::from_rgb(255, 215, 0)
            } else if is_hovered {
                Color32::from_rgb(52, 152, 219)
            } else {
                Color32::from_rgb(36, 44, 56)
            };

            painter.circle_stroke(p, radius + 3.0, Stroke::new(3.0_f32, halo_color));
            painter.circle_filled(p, radius, core_fill);
            painter.circle_stroke(p, radius, Stroke::new(1.5_f32, Color32::WHITE));

            let text_color = if is_selected {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            painter.text(
                p,
                egui::Align2::CENTER_CENTER,
                format!("{}", s.population),
                egui::FontId::proportional((radius * 0.42).clamp(10.0, 18.0)),
                text_color,
            );

            painter.text(
                Pos2::new(p.x, p.y + radius + 6.0),
                egui::Align2::CENTER_TOP,
                format!("{} (#{})", s.name, s.id.0),
                egui::FontId::proportional(12.0),
                Color32::from_rgb(230, 230, 230),
            );

            // Click & double-click detection on settlement node
            if is_hovered {
                if response.double_clicked() {
                    self.selected_settlement = Some(*sid);
                    self.active_tab = ViewTab::SettlementInspector;
                } else if response.clicked() {
                    self.selected_settlement = Some(*sid);
                }
            }
        }

        if let Some((sid, s, p, radius)) = hovered_settlement {
            let hud_size = Vec2::new(260.0, 175.0);
            let mut hud_pos = Pos2::new(p.x + radius + 14.0, p.y - 40.0);

            if hud_pos.x + hud_size.x > rect.max.x {
                hud_pos.x = p.x - radius - hud_size.x - 14.0;
            }
            if hud_pos.x < rect.min.x {
                hud_pos.x = rect.min.x + 10.0;
            }
            if hud_pos.y + hud_size.y > rect.max.y {
                hud_pos.y = rect.max.y - hud_size.y - 10.0;
            }
            if hud_pos.y < rect.min.y {
                hud_pos.y = rect.min.y + 10.0;
            }

            let hud_rect = Rect::from_min_size(hud_pos, hud_size);
            painter.rect_filled(
                hud_rect,
                6.0,
                Color32::from_rgba_unmultiplied(18, 24, 34, 240),
            );
            painter.rect_stroke(
                hud_rect,
                6.0,
                Stroke::new(1.5_f32, Color32::from_rgb(52, 152, 219)),
            );

            let mut cur_y = hud_pos.y + 10.0;
            let draw_line =
                |painter: &egui::Painter, label: &str, val: &str, color: Color32, y: &mut f32| {
                    painter.text(
                        Pos2::new(hud_pos.x + 12.0, *y),
                        egui::Align2::LEFT_TOP,
                        label,
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(180, 180, 180),
                    );
                    painter.text(
                        Pos2::new(hud_pos.x + hud_size.x - 12.0, *y),
                        egui::Align2::RIGHT_TOP,
                        val,
                        egui::FontId::proportional(12.0),
                        color,
                    );
                    *y += 18.0;
                };

            let status_str = if s.is_crisis() {
                "CRISIS"
            } else if s.is_warning() {
                "STRESS"
            } else {
                "HEALTHY"
            };
            let status_color = if s.is_crisis() {
                Color32::from_rgb(230, 50, 50)
            } else if s.is_warning() {
                Color32::from_rgb(240, 160, 40)
            } else {
                Color32::from_rgb(46, 204, 113)
            };

            painter.text(
                Pos2::new(hud_pos.x + 12.0, cur_y),
                egui::Align2::LEFT_TOP,
                format!("{} (#{})", s.name, sid.0),
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );
            painter.text(
                Pos2::new(hud_pos.x + hud_size.x - 12.0, cur_y),
                egui::Align2::RIGHT_TOP,
                status_str,
                egui::FontId::proportional(12.0),
                status_color,
            );
            cur_y += 24.0;

            let food_inv = *s
                .inventories
                .get(&civitas_core::types::ResourceType::Food)
                .unwrap_or(&0.0);
            let food_days = s.days_of_food();

            draw_line(
                &painter,
                "Population:",
                &format!("{}", s.population),
                Color32::WHITE,
                &mut cur_y,
            );
            draw_line(
                &painter,
                "Housing:",
                &format!(
                    "{}/{} ({:.0}%)",
                    s.occupied_housing,
                    s.housing_capacity,
                    s.housing_utilization() * 100.0
                ),
                Color32::WHITE,
                &mut cur_y,
            );
            draw_line(
                &painter,
                "Granary:",
                &format!("{:.0} ({:.0}d)", food_inv, food_days),
                if food_days < 10.0 {
                    Color32::from_rgb(230, 50, 50)
                } else {
                    Color32::from_rgb(46, 204, 113)
                },
                &mut cur_y,
            );
            draw_line(
                &painter,
                "Treasury:",
                &format!("{:.1} (Tax {:.0}%)", s.treasury, s.policy.tax_rate * 100.0),
                if s.treasury < 250.0 {
                    Color32::from_rgb(230, 50, 50)
                } else {
                    Color32::from_rgb(46, 204, 113)
                },
                &mut cur_y,
            );
            draw_line(
                &painter,
                "Avg Wage:",
                &format!("{:.2} coins", s.average_wage()),
                Color32::from_rgb(240, 200, 50),
                &mut cur_y,
            );
            draw_line(
                &painter,
                "Relief / Migration:",
                &format!(
                    "{} / {:+}",
                    if s.policy.emergency_relief {
                        "On"
                    } else {
                        "Off"
                    },
                    s.net_migration
                ),
                Color32::from_rgb(100, 200, 255),
                &mut cur_y,
            );

            painter.text(
                Pos2::new(hud_pos.x + hud_size.x / 2.0, cur_y + 4.0),
                egui::Align2::CENTER_TOP,
                "[Double-click: Inspect | Click: Select]",
                egui::FontId::proportional(10.0),
                Color32::from_rgb(130, 150, 170),
            );
        }
    }

    fn render_settlement_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settlement Inspector");
        ui.separator();

        let s_id = match self.selected_settlement {
            Some(id) => id,
            None => {
                ui.label("No settlement selected. Click a settlement on the World Map.");
                return;
            }
        };

        let s_dir = &self.snapshot.settlements;
        let settlement = match s_dir.get(s_id) {
            Some(s) => s,
            None => {
                ui.label("Selected settlement not found.");
                return;
            }
        };

        if self.policy_synced_settlement != Some(s_id) {
            self.policy_tax_rate = settlement.policy.tax_rate;
            self.policy_emergency_relief = settlement.policy.emergency_relief;
            self.policy_wage_bonus = settlement.policy.wage_bonus;
            self.policy_synced_settlement = Some(s_id);
        }

        ui.horizontal(|ui| {
            ui.label(format!("Name: {}", settlement.name));
            ui.label(format!("ID: #{}", settlement.id.0));
            ui.label(format!("Coordinates: ({}, {})", settlement.x, settlement.y));
            ui.separator();
            ui.label(format!("Fertility: {:.2}x", settlement.fertility_factor));
            ui.label(format!("Timber: {:.2}x", settlement.timber_factor));
            ui.label(format!("Minerals: {:.2}x", settlement.mineral_factor));
        });

        ui.separator();
        ui.group(|ui| {
            ui.heading("Municipal Policy & Governance Controls");
            ui.horizontal(|ui| {
                ui.label("Municipal Tax Rate:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.policy_tax_rate, 0.0..=0.50)
                            .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)),
                    )
                    .changed()
                {
                    self.worker.send_command(SimCommand::SetSettlementPolicy {
                        settlement_id: s_id,
                        policy: SettlementPolicy {
                            tax_rate: self.policy_tax_rate,
                            emergency_relief: self.policy_emergency_relief,
                            wage_bonus: self.policy_wage_bonus,
                        },
                    });
                }

                ui.separator();
                if ui
                    .checkbox(&mut self.policy_emergency_relief, "Emergency Bread Dole")
                    .changed()
                {
                    self.worker.send_command(SimCommand::SetSettlementPolicy {
                        settlement_id: s_id,
                        policy: SettlementPolicy {
                            tax_rate: self.policy_tax_rate,
                            emergency_relief: self.policy_emergency_relief,
                            wage_bonus: self.policy_wage_bonus,
                        },
                    });
                }

                ui.separator();
                ui.label("Wage Bonus:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.policy_wage_bonus, -0.50..=1.00)
                            .custom_formatter(|n, _| format!("{:+.0}%", n * 100.0)),
                    )
                    .changed()
                {
                    self.worker.send_command(SimCommand::SetSettlementPolicy {
                        settlement_id: s_id,
                        policy: SettlementPolicy {
                            tax_rate: self.policy_tax_rate,
                            emergency_relief: self.policy_emergency_relief,
                            wage_bonus: self.policy_wage_bonus,
                        },
                    });
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let timber = *settlement.inventories.get(&ResourceType::Timber).unwrap_or(&0.0);
                let stone = *settlement.inventories.get(&ResourceType::Stone).unwrap_or(&0.0);
                let can_build = settlement.treasury >= 50.0 && timber >= 10.0 && stone >= 5.0;
                let btn = egui::Button::new("Commission Housing Expansion (+25 capacity)");
                if ui.add_enabled(can_build, btn).clicked() {
                    self.worker
                        .send_command(SimCommand::CommissionHousing { settlement_id: s_id });
                }
                ui.label(format!(
                    "Cost: 50.0 coins, 10.0 timber, 5.0 stone | Available: {:.1} coins, {:.1} timber, {:.1} stone",
                    settlement.treasury, timber, stone
                ));
            });
        });

        ui.separator();
        ui.columns(3, |cols| {
            cols[0].group(|ui| {
                ui.heading("Demographics & Housing");
                ui.label(format!("Living Population: {}", settlement.population));
                ui.label(format!("Housing Capacity: {}", settlement.housing_capacity));
                ui.label(format!("Occupied Housing: {}", settlement.occupied_housing));
                ui.label(format!(
                    "Housing Utilization: {:.1}%",
                    settlement.housing_utilization() * 100.0
                ));
                ui.label(format!(
                    "Total Historical Births: {}",
                    settlement.total_births
                ));
                ui.label(format!(
                    "Total Historical Deaths: {}",
                    settlement.total_deaths
                ));
                ui.label(format!(
                    "Net Migration Balance: {:+}",
                    settlement.net_migration
                ));
            });

            cols[1].group(|ui| {
                ui.heading("Economy & Treasury");
                ui.label(format!(
                    "Municipal Treasury: {:.1} coins",
                    settlement.treasury
                ));
                ui.label(format!(
                    "Prevailing Average Wage: {:.2} coins/tick",
                    settlement.average_wage()
                ));
                ui.separator();
                ui.label("Market Prices (Coins):");
                for res in ResourceType::ALL {
                    ui.label(format!("  {:?}: {:.2}", res, settlement.get_price(res)));
                }
            });

            cols[2].group(|ui| {
                ui.heading("Warehouse Reserves");
                for res in ResourceType::ALL {
                    let inv = settlement.inventories.get(&res).unwrap_or(&0.0);
                    ui.label(format!("  {:?}: {:.1} units", res, inv));
                }
                ui.separator();
                ui.heading("Labor Allocation");
                for occ in OccupationType::ALL {
                    let count = settlement.job_headcounts.get(&occ).unwrap_or(&0);
                    ui.label(format!("  {:?}: {}", occ, count));
                }
            });
        });

        ui.separator();
        ui.heading("Resident Households");
        let h_dir = Arc::clone(&self.snapshot.households);
        let mut resident_hhs: Vec<&Household> = h_dir
            .households
            .values()
            .filter(|h| h.settlement_id == s_id)
            .collect();
        resident_hhs.sort_by_key(|h| h.id.0);

        ui.label(format!(
            "Total Households in Settlement: {}",
            resident_hhs.len()
        ));

        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("settlement_resident_hhs_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Household");
                        ui.label("Head Citizen");
                        ui.label("Members");
                        ui.label("Savings");
                        ui.label("Food Reserve");
                        ui.label("Action");
                        ui.end_row();

                        let mut to_inspect_citizen = None;
                        let mut to_inspect_hh = None;

                        for hh in resident_hhs.iter().take(50) {
                            ui.label(format!("#{}", hh.id.0));
                            if ui.link(format!("Citizen #{}", hh.head.0)).clicked() {
                                to_inspect_citizen = Some(hh.head);
                            }
                            ui.label(format!("{}", hh.members.len()));
                            ui.label(format!("{:.1} coins", hh.savings));
                            ui.label(format!("{:.1} units", hh.food_reserve));
                            if ui.button("Inspect").clicked() {
                                to_inspect_hh = Some(hh.id);
                            }
                            ui.end_row();
                        }

                        if let Some(cid) = to_inspect_citizen {
                            self.selected_citizen = Some(cid);
                            self.request_citizen_details(cid);
                            self.active_tab = ViewTab::CitizenInspector;
                        }
                        if let Some(hid) = to_inspect_hh {
                            self.selected_household = Some(hid);
                            self.active_tab = ViewTab::HouseholdInspector;
                        }
                    });
            });
    }

    fn render_citizen_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Citizen Inspector");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Search Citizen ID:");
            let resp = ui.text_edit_singleline(&mut self.citizen_search_text);
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
                || ui.button("Inspect").clicked()
            {
                if let Ok(id) = self.citizen_search_text.trim().parse::<u64>() {
                    self.request_citizen_details(CitizenId(id));
                }
            }
        });

        ui.separator();

        if let Some(details) = self.inspected_citizen_details.clone() {
            let m = &details.meta;
            let d = &details.demographics;
            let hr = &details.household_ref;
            let sr = &details.settlement_ref;
            let o = &details.occupation;
            let f = &details.finances;
            let n = &details.needs;
            let mob = &details.mobility;
            let k = &details.kinship;
            let c = &details.causal_audit;

            ui.columns(2, |cols| {
                cols[0].group(|ui| {
                    ui.heading(format!("Citizen #{}", m.id.0));
                    ui.label(format!(
                        "Vital Status: {}",
                        if m.alive {
                            "Alive & Active"
                        } else {
                            "Deceased (Historical Record)"
                        }
                    ));
                    ui.label(format!("Gender: {:?}", m.gender));
                    ui.label(format!(
                        "Age: {} years ({} ticks)",
                        d.age_years, d.age_ticks
                    ));
                    ui.label(format!("Health: {}/100", d.health));
                    ui.label(format!("Satiety: {}/100", n.satiety));
                    ui.label(format!("Shelter: {}/100", n.shelter));
                    ui.label(format!("Comfort: {}/100", n.comfort));
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Settlement:");
                        if ui.link(format!("Settlement #{}", sr.settlement_id.0)).clicked() {
                            self.selected_settlement = Some(sr.settlement_id);
                            self.active_tab = ViewTab::SettlementInspector;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(format!("Household: #{} ({:?})", hr.household_id.0, hr.role));
                        if ui.button("Inspect Household").clicked() {
                            self.selected_household = Some(hr.household_id);
                            self.active_tab = ViewTab::HouseholdInspector;
                        }
                    });
                    ui.separator();
                    ui.heading("Kinship & Lineage");
                    let mut target_inspect_citizen = None;

                    ui.horizontal(|ui| {
                        ui.label("Spouse:");
                        if let Some(spouse_id) = k.spouse {
                            if ui.link(format!("Citizen #{}", spouse_id.0)).clicked() {
                                target_inspect_citizen = Some(spouse_id);
                            }
                        } else {
                            ui.label("None (Unmarried)");
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Parent A:");
                        if let Some(p_id) = k.parent_a {
                            if ui.link(format!("Citizen #{}", p_id.0)).clicked() {
                                target_inspect_citizen = Some(p_id);
                            }
                        } else {
                            ui.label("None (Founding generation)");
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Parent B:");
                        if let Some(p_id) = k.parent_b {
                            if ui.link(format!("Citizen #{}", p_id.0)).clicked() {
                                target_inspect_citizen = Some(p_id);
                            }
                        } else {
                            ui.label("None (Founding generation)");
                        }
                    });

                    ui.label(format!("Total Children: {}", k.children_count));

                    if let Some(target_cid) = target_inspect_citizen {
                        self.selected_citizen = Some(target_cid);
                        self.citizen_search_text = target_cid.0.to_string();
                        self.request_citizen_details(target_cid);
                    }
                });

                cols[1].group(|ui| {
                    ui.heading("Economic & Behavioral Profile");
                    ui.label(format!("Occupation: {:?}", o.occupation));
                    ui.label(format!(
                        "Skill Level: {} (Experience: {})",
                        o.skill_level, o.experience
                    ));
                    ui.label(format!("Productivity Multiplier: {:.2}x", o.productivity));
                    ui.separator();
                    ui.label(format!("Personal Savings: {:.1} coins", f.savings));
                    ui.label(format!("Last Daily Income: {:.1} coins", f.last_income));
                    ui.label(format!("Last Daily Expense: {:.1} coins", f.last_expense));
                    ui.separator();
                    ui.label(format!(
                        "Mobility Status: {}",
                        match mob.status {
                            civitas_core::types::MigrationStatus::Settled => "Settled (Resident)".to_string(),
                            civitas_core::types::MigrationStatus::InTransit { origin, destination, ticks_remaining, total_ticks } => {
                                format!("In Transit (From Settlement #{} to #{} - {}/{} ticks remaining)", origin.0, destination.0, ticks_remaining, total_ticks)
                            }
                        }
                    ));
                    ui.separator();
                    ui.heading("Causal Audit (\"Why Did This Happen?\")");
                    ui.label(format!("Last Decision Reason: {:?}", c.trace.reason));
                    ui.label(format!("Decision Occurred on Tick: {}", c.trace.tick));
                    ui.label(format!("Primary Parameter: {:.2}", c.trace.primary_metric));
                    ui.label(format!("Secondary Parameter: {:.2}", c.trace.secondary_metric));
                    ui.separator();
                    ui.label("Structured Human Explanation:");
                    ui.colored_label(
                        Color32::from_rgb(100, 220, 150),
                        c.trace.to_human_explanation(),
                    );
                });
            });
        } else {
            ui.label(format!(
                "Citizen #{} not found or details awaiting worker response.",
                self.selected_citizen.map_or(0, |id| id.0)
            ));
        }
    }

    fn render_household_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Household Inspector");
        ui.separator();

        let h_dir = Arc::clone(&self.snapshot.households);
        ui.label(format!(
            "Total Authoritative Households: {}",
            h_dir.households.len()
        ));

        // Filter and pagination controls (CIV-12)
        ui.horizontal(|ui| {
            ui.label("Search Household ID:");
            ui.text_edit_singleline(&mut self.household_search_text);
            if ui.button("Jump").clicked() {
                if let Ok(id) = self.household_search_text.trim().parse::<u32>() {
                    self.selected_household = Some(HouseholdId(id));
                }
            }
        });

        let target_hh_id = self.selected_household.unwrap_or(HouseholdId(1));
        if let Some(hh) = h_dir.get(target_hh_id) {
            ui.group(|ui| {
                ui.heading(format!("Household #{}", hh.id.0));
                ui.horizontal(|ui| {
                    ui.label("Settlement Location:");
                    if ui
                        .link(format!("Settlement #{}", hh.settlement_id.0))
                        .clicked()
                    {
                        self.selected_settlement = Some(hh.settlement_id);
                        self.active_tab = ViewTab::SettlementInspector;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(format!("Head Citizen: #{}", hh.head.0));
                    if ui.button("Inspect Head").clicked() {
                        self.request_citizen_details(hh.head);
                        self.active_tab = ViewTab::CitizenInspector;
                    }
                });
                ui.label(format!("Treasury: {:.1} coins", hh.savings));
                ui.label(format!("Food Reserves: {:.1} units", hh.food_reserve));
                ui.label(format!("Migration Pressure: {:.2}", hh.migration_pressure));
                ui.separator();
                ui.heading(format!("Living Members ({})", hh.members.len()));
                for &cid in &hh.members {
                    ui.horizontal(|ui| {
                        if ui.link(format!("Member Citizen #{}", cid.0)).clicked() {
                            self.request_citizen_details(cid);
                            self.active_tab = ViewTab::CitizenInspector;
                        }
                    });
                }
            });
        } else {
            ui.label(format!(
                "Household #{} not found (may have been dissolved).",
                target_hh_id.0
            ));
        }

        ui.separator();
        ui.heading("Households Directory Browse");

        let items_per_page = 15;
        let total_hh = h_dir.households.len();
        let max_pages = (total_hh / items_per_page).max(1);

        ui.horizontal(|ui| {
            if ui.button("Prev Page").clicked() && self.household_page > 0 {
                self.household_page -= 1;
            }
            ui.label(format!("Page {} of {}", self.household_page + 1, max_pages));
            if ui.button("Next Page").clicked() && (self.household_page + 1) < max_pages {
                self.household_page += 1;
            }
        });

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                let start = self.household_page * items_per_page;
                for (hid, hh) in h_dir.households.iter().skip(start).take(items_per_page) {
                    let is_sel = self.selected_household == Some(*hid);
                    let label =
                        format!(
                    "Household #{} - Settlement #{} - {} members - {:.1} coins - {:.1} food",
                    hid.0, hh.settlement_id.0, hh.members.len(), hh.savings, hh.food_reserve
                );
                    if ui.selectable_label(is_sel, label).clicked() {
                        self.selected_household = Some(*hid);
                    }
                }
            });
    }

    fn render_analytics(&mut self, ui: &mut egui::Ui) {
        ui.heading("Civilization Analytics & Emergent Trends");
        ui.separator();

        ui.columns(2, |cols| {
            cols[0].group(|ui| {
                ui.heading("Living Population Trajectory");
                let points: PlotPoints = self
                    .history_ticks
                    .iter()
                    .zip(&self.history_living_pop)
                    .map(|(&t, &p)| [t, p])
                    .collect();
                let line = Line::new(points)
                    .name("Living Citizens")
                    .color(Color32::from_rgb(100, 200, 255));
                Plot::new("pop_plot").height(250.0).show(ui, |plot_ui| {
                    plot_ui.line(line);
                });
            });

            cols[1].group(|ui| {
                ui.heading("Commodity Food Price & Wage Dynamics");
                let price_points: PlotPoints = self
                    .history_ticks
                    .iter()
                    .zip(&self.history_avg_food_price)
                    .map(|(&t, &p)| [t, p])
                    .collect();
                let wage_points: PlotPoints = self
                    .history_ticks
                    .iter()
                    .zip(&self.history_avg_wage)
                    .map(|(&t, &w)| [t, w])
                    .collect();

                let price_line = Line::new(price_points)
                    .name("Average Food Price")
                    .color(Color32::from_rgb(255, 120, 100));
                let wage_line = Line::new(wage_points)
                    .name("Average Daily Wage")
                    .color(Color32::from_rgb(100, 255, 150));

                Plot::new("econ_plot").height(250.0).show(ui, |plot_ui| {
                    plot_ui.line(price_line);
                    plot_ui.line(wage_line);
                });
            });
        });
    }

    fn render_event_log(&mut self, ui: &mut egui::Ui) {
        ui.heading("Authoritative Event Ring");
        ui.label(format!(
            "Displaying {} Recent Authoritative Events",
            self.snapshot.recent_events.len()
        ));
        ui.separator();

        let recent_events = self.snapshot.recent_events.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for event in &recent_events {
                ui.horizontal(|ui| {
                    ui.label(format!("[Tick {:4}]", event.tick));
                    let color = match event.event_type {
                        civitas_core::events::SimEventType::Birth => {
                            Color32::from_rgb(100, 255, 100)
                        }
                        civitas_core::events::SimEventType::Death => {
                            Color32::from_rgb(255, 100, 100)
                        }
                        civitas_core::events::SimEventType::Marriage => {
                            Color32::from_rgb(255, 150, 220)
                        }
                        civitas_core::events::SimEventType::HouseholdDissolved => {
                            Color32::from_rgb(200, 150, 100)
                        }
                        civitas_core::events::SimEventType::MigrationStart
                        | civitas_core::events::SimEventType::MigrationArrival => {
                            Color32::from_rgb(255, 215, 0)
                        }
                        civitas_core::events::SimEventType::PriceShock => {
                            Color32::from_rgb(255, 160, 50)
                        }
                        civitas_core::events::SimEventType::SettlementExpansion => {
                            Color32::from_rgb(100, 220, 255)
                        }
                        _ => Color32::WHITE,
                    };
                    ui.colored_label(color, format!("{:?}", event.event_type));
                    if let Some(cid) = event.entity {
                        if ui.link(format!("Citizen #{}", cid.0)).clicked() {
                            self.request_citizen_details(cid);
                            self.active_tab = ViewTab::CitizenInspector;
                        }
                    }
                    if let Some(sid) = event.settlement {
                        if ui.link(format!("at Settlement #{}", sid.0)).clicked() {
                            self.selected_settlement = Some(sid);
                            self.active_tab = ViewTab::SettlementInspector;
                        }
                    }
                    ui.label(format!("- {}", event.reason.to_human_explanation()));
                });
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let config = parse_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("CIVITAS-1M: Frontier Civilization Simulator"),
        ..Default::default()
    };

    eframe::run_native(
        "CIVITAS-1M",
        options,
        Box::new(move |cc| Ok(Box::new(CivitasApp::new(cc, config)))),
    )
}
