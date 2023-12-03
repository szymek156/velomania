use egui::Widget;

#[derive(Default)]
pub struct Gui {
    selected: bool,
}

impl Gui {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self::default()
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Hello World!");

                ui.horizontal(|ui| {
                    ui.add(PowerCadenceTime::new("100", "150", &80, "01:21:15"));

                    ui.add(Session::new(
                        &vec![
                            "step1", "step2", "step3", "step4", "step5", "step6", "step7", "step8",
                            "step9",
                        ],
                        &3,
                    ));
                });

                ui.checkbox(&mut self.selected, "heyy")
            })
        });
    }
}

struct PowerCadenceTime<'a> {
    set_power: &'a str,
    current_power: &'a str,
    cadence: &'a u32,
    training_time: &'a str,
}

impl<'a> PowerCadenceTime<'a> {
    fn new(
        set_power: &'a str,
        current_power: &'a str,
        cadence: &'a u32,
        training_time: &'a str,
    ) -> Self {
        Self {
            set_power,
            current_power,
            cadence,
            training_time,
        }
    }
}

impl<'a> Widget for PowerCadenceTime<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.label("POWER");
                ui.label("POWER SET");
            });

            ui.horizontal(|ui| {
                ui.label(self.current_power);
                ui.label(self.set_power)
            });

            ui.horizontal(|ui| {
                ui.label("CADENCE");
                ui.label("TRAIN TIME")
            });

            ui.horizontal(|ui| {
                ui.label(self.cadence.to_string());
                ui.label(self.training_time)
            })
            .inner
        })
        .inner
    }
}

struct Session<'a> {
    steps: &'a Vec<&'a str>,
    current_step: &'a usize,
}

impl<'a> Session<'a> {
    fn new(steps: &'a Vec<&'a str>, current_step: &'a usize) -> Self {
        Self {
            steps,
            current_step,
        }
    }
}

impl<'a> Widget for Session<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical_centered(|ui| {
            ui.label("SESSION");
            egui::Grid::new("session_grid")
                .num_columns(1)
                .show(ui, |ui| {
                    let mut resp;
                    for (idx, step) in self.steps.iter().enumerate() {
                        if &idx == self.current_step {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, *step);
                        } else {
                            resp = ui.label(*step);
                        }

                        ui.end_row();
                    }

                    ui.label("--")
                })
                .inner
        })
        .inner
    }
}
