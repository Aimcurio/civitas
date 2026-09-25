use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use egui_plot::{Line, Plot, PlotPoints};
use std::time::Instant;

use civitas_core::components::{
    CausalAudit, CitizenMeta, Demographics, HouseholdRef, Kinship, MobilityProfile,
    OccupationProfile, PersonalFinances, PhysicalNeeds, SettlementRef,
};
use civitas_core::events::EventRing;
use civitas_core::household::HouseholdDirectory;
use civitas_core::persistence::{load_from_file, save_to_file};
use civitas_core::settlement::SettlementDirectory;
use civitas_core::sim::Simulation;
use civitas_core::types::{
    Biome, CitizenId, HouseholdId, OccupationType, ResourceType, SettlementId, SimClock,
};
use civitas_core::world::WorldMap;

#[derive(PartialEq, Eq, Clone, Copy)]
enum ViewTab {
    WorldMap,
    SettlementInspector,
    CitizenInspector,
    HouseholdInspector,
    Analytics,
    EventLog,
}

struct CivitasApp {
    sim: Simulation,
    is_running: bool,
    speed: u32,
    selected_settlement: Option<SettlementId>,
    selected_citizen: Option<CitizenId>,
    selected_household: Option<HouseholdId>,
    citizen_search_text: String,
    save_path_text: String,
    status_message: String,
    active_tab: ViewTab,

    // Time-series history for analytics
    history_ticks: Vec<f64>,
    history_living_pop: Vec<f64>,
    history_avg_food_price: Vec<f64>,
    history_avg_wage: Vec<f64>,

    // Performance telemetry
    measured_tps: f64,

    // Camera
    camera_offset: Vec2,
    camera_zoom: f32,

    // World creation parameters
    new_seed: u64,
    new_population: usize,
}

impl CivitasApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let seed = 42;
        let population = 5_000;
        let sim = Simulation::new(seed, 128, 128, 8, population);

        let mut app = Self {
            sim,
            is_running: false,
            speed: 1,
            selected_settlement: Some(SettlementId(0)),
            selected_citizen: Some(CitizenId(1)),
            selected_household: Some(HouseholdId(1)),
            citizen_search_text: "1".to_string(),
            save_path_text: "world_save.civ".to_string(),
            status_message: "Ready. Simulation paused.".to_string(),
            active_tab: ViewTab::WorldMap,
            history_ticks: Vec::new(),
            history_living_pop: Vec::new(),
            history_avg_food_price: Vec::new(),
            history_avg_wage: Vec::new(),
            measured_tps: 0.0,
            camera_offset: Vec2::ZERO,
            camera_zoom: 1.0,
            new_seed: 12345,
            new_population: 5_000,
        };

        app.record_analytics();
        app
    }

    fn record_analytics(&mut self) {
        let tick = self.sim.world.resource::<SimClock>().tick as f64;
        let living = self.sim.living_citizens_count() as f64;

        let s_dir = self.sim.world.resource::<SettlementDirectory>();
        let mut total_food_price = 0.0;
        let mut total_wage = 0.0;
        let count = s_dir.settlements.len().max(1) as f64;

        for s in s_dir.settlements.values() {
            total_food_price += s.get_price(ResourceType::Food) as f64;
            total_wage += s.average_wage() as f64;
        }

        self.history_ticks.push(tick);
        self.history_living_pop.push(living);
        self.history_avg_food_price.push(total_food_price / count);
        self.history_avg_wage.push(total_wage / count);

        // Keep maximum 200 data points for UI efficiency
        if self.history_ticks.len() > 200 {
            self.history_ticks.remove(0);
            self.history_living_pop.remove(0);
            self.history_avg_food_price.remove(0);
            self.history_avg_wage.remove(0);
        }
    }

    fn step_sim(&mut self) {
        let start = Instant::now();
        self.sim.step();
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.measured_tps = 1.0 / elapsed;
        }
        self.record_analytics();
    }
}

impl eframe::App for CivitasApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Automatic stepping if running
        if self.is_running {
            for _ in 0..self.speed {
                self.step_sim();
            }
            ctx.request_repaint();
        }

        // Top Control Panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("CIVITAS-1M");
                ui.separator();

                // Simulation Controls
                if ui
                    .button(if self.is_running {
                        "⏸ Pause"
                    } else {
                        "▶ Play"
                    })
                    .clicked()
                {
                    self.is_running = !self.is_running;
                    self.status_message = if self.is_running {
                        "Simulation running.".to_string()
                    } else {
                        "Simulation paused.".to_string()
                    };
                }

                if ui.button("⏭ Step 1").clicked() {
                    self.is_running = false;
                    self.step_sim();
                    self.status_message = format!(
                        "Advanced 1 tick. Current tick: {}",
                        self.sim.world.resource::<SimClock>().tick
                    );
                }

                ui.separator();
                ui.label("Speed:");
                for &s in &[1, 2, 5, 10, 50] {
                    if ui
                        .selectable_label(self.speed == s, format!("{}x", s))
                        .clicked()
                    {
                        self.speed = s;
                    }
                }

                ui.separator();
                let tick = self.sim.world.resource::<SimClock>().tick;
                let living = self.sim.living_citizens_count();
                let total = self.sim.total_citizens_count();
                ui.label(format!("Tick: {}", tick));
                ui.label(format!("Population: {}/{}", living, total));
                ui.label(format!("TPS: {:.1}", self.measured_tps));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Verify Invariants").clicked() {
                        match self.sim.check_invariants() {
                            Ok(_) => {
                                self.status_message = "All invariants 100% verified!".to_string()
                            }
                            Err(e) => {
                                self.status_message =
                                    format!("Invariant violation: {} errors!", e.len())
                            }
                        }
                    }
                });
            });

            ui.separator();

            // View Tabs
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ViewTab::WorldMap, "🗺 World Map");
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::SettlementInspector,
                    "🏛 Settlements",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::CitizenInspector,
                    "👤 Citizen Inspector",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::HouseholdInspector,
                    "🏠 Households",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ViewTab::Analytics,
                    "📈 Analytics & Charts",
                );
                ui.selectable_value(&mut self.active_tab, ViewTab::EventLog, "📜 Event Log");
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

                if ui.button("🔄 Generate New World").clicked() {
                    self.is_running = false;
                    self.sim = Simulation::new(self.new_seed, 128, 128, 8, self.new_population);
                    self.selected_settlement = Some(SettlementId(0));
                    self.selected_citizen = Some(CitizenId(1));
                    self.history_ticks.clear();
                    self.history_living_pop.clear();
                    self.history_avg_food_price.clear();
                    self.history_avg_wage.clear();
                    self.record_analytics();
                    self.status_message = format!(
                        "Generated new world with seed {} (pop: {})",
                        self.new_seed, self.new_population
                    );
                }

                ui.separator();
                ui.heading("Save / Load");
                ui.horizontal(|ui| {
                    ui.label("File:");
                    ui.text_edit_singleline(&mut self.save_path_text);
                });

                ui.horizontal(|ui| {
                    if ui.button("💾 Save State").clicked() {
                        let seed = self.sim.seed;
                        match save_to_file(&mut self.sim.world, seed, &self.save_path_text) {
                            Ok(_) => {
                                self.status_message =
                                    format!("Saved snapshot to {}", self.save_path_text)
                            }
                            Err(e) => self.status_message = format!("Save error: {}", e),
                        }
                    }

                    if ui.button("📂 Load State").clicked() {
                        match load_from_file(&self.save_path_text) {
                            Ok(snapshot) => {
                                self.is_running = false;
                                self.sim = Simulation::from_snapshot(snapshot);
                                self.status_message =
                                    format!("Loaded snapshot from {}", self.save_path_text);
                                self.record_analytics();
                            }
                            Err(e) => self.status_message = format!("Load error: {}", e),
                        }
                    }
                });

                ui.separator();
                ui.heading("Settlements Quick List");
                let s_dir = self.sim.world.resource::<SettlementDirectory>().clone();
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
        ui.label("Pan with drag, zoom with scroll. Click settlement nodes to inspect.");

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
        painter.rect_filled(rect, 0.0, Color32::from_rgb(25, 30, 40));

        let world_map = self.sim.world.resource::<WorldMap>().clone();
        let settlements = self.sim.world.resource::<SettlementDirectory>().clone();

        let center = rect.center() + self.camera_offset;
        let scale = 4.0 * self.camera_zoom;

        let map_to_screen = |mx: u32, my: u32| -> Pos2 {
            Pos2::new(
                center.x + ((mx as f32) - (world_map.width as f32) / 2.0) * scale,
                center.y + ((my as f32) - (world_map.height as f32) / 2.0) * scale,
            )
        };

        // Draw low-res terrain representation (sampled every 4 tiles for high performance)
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
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(200, 200, 200, 40)),
                    );
                }
            }
        }

        // Draw settlement nodes
        for (sid, s) in &settlements.settlements {
            let p = map_to_screen(s.x, s.y);
            let radius = ((s.population as f32).sqrt() * 0.8 * self.camera_zoom).clamp(6.0, 30.0);

            let is_selected = self.selected_settlement == Some(*sid);
            let fill_color = if is_selected {
                Color32::from_rgb(255, 215, 0)
            } else {
                Color32::from_rgb(220, 80, 80)
            };

            painter.circle_filled(p, radius, fill_color);
            painter.circle_stroke(p, radius, Stroke::new(2.0_f32, Color32::WHITE));

            // Name label
            painter.text(
                Pos2::new(p.x, p.y + radius + 3.0),
                egui::Align2::CENTER_TOP,
                format!("{} ({})", s.name, s.population),
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );

            // Interaction check for clicking settlement node
            if response.clicked() {
                if let Some(mouse_pos) = response.interact_pointer_pos() {
                    if p.distance(mouse_pos) <= radius + 5.0 {
                        self.selected_settlement = Some(*sid);
                    }
                }
            }
        }
    }

    fn render_settlement_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settlement Inspector");
        ui.separator();

        let s_id = match self.selected_settlement {
            Some(id) => id,
            None => {
                ui.label("No settlement selected.");
                return;
            }
        };

        let s_dir = self.sim.world.resource::<SettlementDirectory>();
        let settlement = match s_dir.get(s_id) {
            Some(s) => s.clone(),
            None => {
                ui.label("Selected settlement not found.");
                return;
            }
        };

        ui.horizontal(|ui| {
            ui.label(format!("🏛 Name: {}", settlement.name));
            ui.label(format!("ID: {}", settlement.id.0));
            ui.label(format!("Location: ({}, {})", settlement.x, settlement.y));
        });

        ui.separator();
        ui.columns(3, |cols| {
            cols[0].group(|ui| {
                ui.heading("Demographics & Housing");
                ui.label(format!("Population: {}", settlement.population));
                ui.label(format!("Housing Capacity: {}", settlement.housing_capacity));
                ui.label(format!("Occupied Housing: {}", settlement.occupied_housing));
                ui.label(format!(
                    "Housing Utilization: {:.1}%",
                    settlement.housing_utilization() * 100.0
                ));
                ui.label(format!("Total Births: {}", settlement.total_births));
                ui.label(format!("Total Deaths: {}", settlement.total_deaths));
                ui.label(format!("Net Migration: {:+}", settlement.net_migration));
            });

            cols[1].group(|ui| {
                ui.heading("Economy & Treasury");
                ui.label(format!("Treasury: {:.1} coins", settlement.treasury));
                ui.label(format!(
                    "Average Wage: {:.2} coins",
                    settlement.average_wage()
                ));
                ui.separator();
                ui.label("Market Prices:");
                for res in ResourceType::ALL {
                    ui.label(format!(
                        "  {:?}: {:.2} coins",
                        res,
                        settlement.get_price(res)
                    ));
                }
            });

            cols[2].group(|ui| {
                ui.heading("Warehouse Inventories");
                for res in ResourceType::ALL {
                    let inv = settlement.inventories.get(&res).unwrap_or(&0.0);
                    ui.label(format!("  {:?}: {:.1} units", res, inv));
                }
                ui.separator();
                ui.heading("Labor Headcounts");
                for occ in OccupationType::ALL {
                    let count = settlement.job_headcounts.get(&occ).unwrap_or(&0);
                    ui.label(format!("  {:?}: {}", occ, count));
                }
            });
        });
    }

    fn render_citizen_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Citizen Inspector");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Search Citizen ID:");
            ui.text_edit_singleline(&mut self.citizen_search_text);
            if ui.button("Inspect").clicked() {
                if let Ok(id) = self.citizen_search_text.trim().parse::<u64>() {
                    self.selected_citizen = Some(CitizenId(id));
                }
            }
        });

        ui.separator();

        let target_id = match self.selected_citizen {
            Some(id) => id,
            None => {
                ui.label("Enter an ID or select a citizen.");
                return;
            }
        };

        let mut query = self.sim.world.query::<(
            &CitizenMeta,
            &Demographics,
            &HouseholdRef,
            &SettlementRef,
            &OccupationProfile,
            &PersonalFinances,
            &PhysicalNeeds,
            &MobilityProfile,
            &Kinship,
            &CausalAudit,
        )>();

        let mut found = None;
        for (m, d, hr, sr, o, f, n, mob, k, c) in query.iter(&self.sim.world) {
            if m.id == target_id {
                found = Some((
                    m.clone(),
                    d.clone(),
                    hr.clone(),
                    sr.clone(),
                    o.clone(),
                    f.clone(),
                    n.clone(),
                    mob.clone(),
                    k.clone(),
                    c.clone(),
                ));
                break;
            }
        }

        if let Some((m, d, hr, sr, o, f, n, mob, k, c)) = found {
            ui.columns(2, |cols| {
                cols[0].group(|ui| {
                    ui.heading(format!("Citizen #{}", m.id.0));
                    ui.label(format!(
                        "Status: {}",
                        if m.alive {
                            "🟢 Alive"
                        } else {
                            "🔴 Deceased"
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
                    ui.separator();
                    ui.label(format!("Settlement: #{}", sr.settlement_id.0));
                    ui.label(format!("Household: #{} ({:?})", hr.household_id.0, hr.role));
                    ui.separator();
                    ui.label("Kinship:");
                    ui.label(format!("  Parent A: {:?}", k.parent_a.map(|id| id.0)));
                    ui.label(format!("  Parent B: {:?}", k.parent_b.map(|id| id.0)));
                    ui.label(format!("  Spouse: {:?}", k.spouse.map(|id| id.0)));
                    ui.label(format!("  Children: {}", k.children_count));
                });

                cols[1].group(|ui| {
                    ui.heading("Economic & Behavioral Profile");
                    ui.label(format!("Occupation: {:?}", o.occupation));
                    ui.label(format!(
                        "Skill Level: {} (Exp: {})",
                        o.skill_level, o.experience
                    ));
                    ui.label(format!("Productivity: {:.2}x", o.productivity));
                    ui.separator();
                    ui.label(format!("Personal Savings: {:.1} coins", f.savings));
                    ui.label(format!("Last Daily Income: {:.1} coins", f.last_income));
                    ui.separator();
                    ui.label(format!("Mobility Status: {:?}", mob.status));
                    ui.separator();
                    ui.heading("Causal Audit (\"Why Did This Happen?\")");
                    ui.label(format!("Last Significant Decision: {:?}", c.trace.reason));
                    ui.label(format!("Decision Tick: {}", c.trace.tick));
                    ui.label(format!("Primary Parameter: {:.2}", c.trace.primary_metric));
                    ui.label(format!(
                        "Secondary Parameter: {:.2}",
                        c.trace.secondary_metric
                    ));
                    ui.separator();
                    ui.label("Structured Human Explanation:");
                    ui.colored_label(
                        Color32::from_rgb(100, 220, 150),
                        c.trace.to_human_explanation(),
                    );
                });
            });
        } else {
            ui.label(format!("Citizen #{} not found.", target_id.0));
        }
    }

    fn render_household_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Household Inspector");
        ui.separator();

        let h_dir = self.sim.world.resource::<HouseholdDirectory>().clone();
        ui.label(format!(
            "Total Registered Households: {}",
            h_dir.households.len()
        ));

        let target_hh = self.selected_household.unwrap_or(HouseholdId(1));

        if let Some(hh) = h_dir.get(target_hh) {
            ui.group(|ui| {
                ui.heading(format!("Household #{}", hh.id.0));
                ui.label(format!("Settlement: #{}", hh.settlement_id.0));
                ui.label(format!("Head Citizen: #{}", hh.head.0));
                ui.label(format!("Pooled Treasury: {:.1} coins", hh.savings));
                ui.label(format!("Food Reserves: {:.1} units", hh.food_reserve));
                ui.label(format!("Migration Pressure: {:.2}", hh.migration_pressure));
                ui.separator();
                ui.label(format!("Members ({}):", hh.members.len()));
                for cid in &hh.members {
                    if ui.button(format!("👤 Inspect Member #{}", cid.0)).clicked() {
                        self.selected_citizen = Some(*cid);
                        self.active_tab = ViewTab::CitizenInspector;
                    }
                }
            });
        } else {
            ui.label("Selected household not found.");
        }
    }

    fn render_analytics(&mut self, ui: &mut egui::Ui) {
        ui.heading("Civilization Analytics & Emergent Trends");
        ui.separator();

        ui.columns(2, |cols| {
            cols[0].group(|ui| {
                ui.label("Living Population Trend Over Time");
                let points: PlotPoints = self
                    .history_ticks
                    .iter()
                    .zip(&self.history_living_pop)
                    .map(|(&t, &p)| [t, p])
                    .collect();
                let line = Line::new(points)
                    .name("Population")
                    .color(Color32::from_rgb(100, 200, 255));
                Plot::new("pop_plot").height(250.0).show(ui, |plot_ui| {
                    plot_ui.line(line);
                });
            });

            cols[1].group(|ui| {
                ui.label("Average Commodity Food Price & Wage Level");
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
                    .name("Food Price")
                    .color(Color32::from_rgb(255, 120, 100));
                let wage_line = Line::new(wage_points)
                    .name("Avg Wage")
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
        let events = self.sim.world.resource::<EventRing>().clone();
        ui.label(format!(
            "Total Historical Events Emitted: {}",
            events.total_emitted
        ));
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for event in events.recent(50) {
                ui.horizontal(|ui| {
                    ui.label(format!("[Tick {:4}]", event.tick));
                    let color = match event.event_type {
                        civitas_core::events::SimEventType::Birth => {
                            Color32::from_rgb(100, 255, 100)
                        }
                        civitas_core::events::SimEventType::Death => {
                            Color32::from_rgb(255, 100, 100)
                        }
                        civitas_core::events::SimEventType::MigrationStart
                        | civitas_core::events::SimEventType::MigrationArrival => {
                            Color32::from_rgb(255, 215, 0)
                        }
                        _ => Color32::WHITE,
                    };
                    ui.colored_label(color, format!("{:?}", event.event_type));
                    if let Some(cid) = event.entity {
                        if ui.link(format!("Citizen #{}", cid.0)).clicked() {
                            self.selected_citizen = Some(cid);
                            self.active_tab = ViewTab::CitizenInspector;
                        }
                    }
                    if let Some(sid) = event.settlement {
                        ui.label(format!("at Settlement #{}", sid.0));
                    }
                    ui.label(format!("— {}", event.reason.to_human_explanation()));
                });
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("CIVITAS-1M: Frontier Civilization Simulator"),
        ..Default::default()
    };

    eframe::run_native(
        "CIVITAS-1M",
        options,
        Box::new(|cc| Ok(Box::new(CivitasApp::new(cc)))),
    )
}
