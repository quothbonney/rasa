
use crate::measurements::MeasurementWindow;
use egui::{CentralPanel, Vec2, Context, SidePanel, Button, Ui};
use egui::plot::{Plot, Values, Polygon, PlotPoints};
use std::sync::{Arc, Mutex};
use egui::Color32;

macro_rules! add_plot_line {
    ($plot_ui:expr, $color:expr, $data:expr, $channel:expr) => {
        {
            let line = egui::plot::Line::new($data.lock().unwrap().plot_values($channel));
            let stroke = egui::Stroke::new(2.0, $color);
            $plot_ui.line(line.stroke(stroke));
        }
    };
}


struct MeasurementPlot {
    measurements: Arc<Mutex<MeasurementWindow>>,
    show_box: bool,
}

impl MeasurementPlot {
    fn new(measurements: Arc<Mutex<MeasurementWindow>>, show_box: bool) -> Self {
        Self {
            measurements,
            show_box,
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        let mut plot = Plot::new("measurements").allow_drag(false);

        plot.show(ui, |plot_ui| {
            add_plot_line!(plot_ui, Color32::LIGHT_GREEN, self.measurements, 0);
            add_plot_line!(plot_ui, Color32::LIGHT_RED, self.measurements, 1);
            // Add more plot lines if needed...

            let series: PlotPoints = PlotPoints::new(self.measurements.lock().unwrap().clone());
            if self.show_box {
                let poly = Polygon::new(series);
                plot_ui.polygon(poly);
            }
        });
    }
}

struct RewardPlot {
    measurements: Arc<Mutex<Measurements>>,
}

impl RewardPlot {
    fn new(measurements: Arc<Mutex<Measurements>>) -> Self {
        Self { measurements }
    }

    fn show(&mut self, ui: &mut Ui) {
        let mut reward_plot = Plot::new("rewards").allow_drag(false);
        reward_plot = reward_plot.include_y(300.0);
        reward_plot = reward_plot.include_y(200.0);

        reward_plot.show(ui, |plot_ui| {
            add_plot_line!(plot_ui, Color32::GOLD, self.measurements, 4);
        });
    }
}

struct Sidebar {
    show_box: bool,
    sidebar_text: String,
}

impl Sidebar {
    fn new(show_box: bool) -> Self {
        Self {
            show_box,
            sidebar_text: String::new(),
        }
    }

    fn show(&mut self, ctx: &CtxRef) {
        SidePanel::right("Sidebar").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label("Sidebar");
                ui.separator();

                if ui.button("Show Box").clicked() {
                    self.show_box = !self.show_box;
                }
                if ui.button("Button 2").clicked() {
                    self.sidebar_text = "Button 2 clicked".to_string();
                }
                if ui.button("Button 3").clicked() {
                    self.sidebar_text = "Button 3 clicked".to_string();
                }
            });
        });
    }
}

pub struct App {
    pub(crate) measurements: Arc<Mutex<Measurements>>,
    measurement_plot: MeasurementPlot,
    reward_plot: RewardPlot,
    sidebar: Sidebar,
    // Other fields...
}

impl App {
    pub fn new(look_behind: usize, channels: usize) -> Self {
        let measurements = Arc::new(Mutex::new(MeasurementWindow::new_with_look_behind(
            look_behind,
            channels
        )));
        let measurement_plot = MeasurementPlot::new(Arc::clone(&measurements), true);
        let reward_plot = RewardPlot::new(Arc::clone(&measurements));
        let sidebar = Sidebar::new(false);

        Self {
            measurements,
            measurement_plot,
            reward_plot,
            sidebar,
            // initialize other fields...
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut egui::Frame) {
        let side_panel_width = 200.0;
        CentralPanel::default().show(ctx, |ui| {
            let total_height = ui.available_size().y;
            let button_ratio = 0.7;
            let label_ratio = 0.3;

            let button_height = total_height * button_ratio;
            let label_height = total_height * label_ratio;

            ui.allocate_ui(Vec2::new(ui.available_size().x - side_panel_width, button_height), |ui| {
                self.measurement_plot.show(ui);
            });

            ui.allocate_ui(Vec2::new(ui.available_size().x - side_panel_width, label_height), |ui| {
                self.reward_plot.show(ui);
            });
        });

        self.sidebar.show(ctx);

        // Make it always repaint.
        ctx.request_repaint();
    }
}
