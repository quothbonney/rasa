use crate::measurements::MeasurementWindow;
use eframe::egui;
use std::io::BufReader;
use std::io::BufRead;
use std::sync::*;
use serialport::*;
use std::thread;
use tracing::{error, info, warn};
use std::f64;
use std::any::type_name;
use clap::{arg, Parser};
use egui::plot::*;
use egui::{Label, Button, Vec2};

use crate::structs::RasaVariables;

macro_rules! add_plot_line {
    ($plot_ui:expr, $color:expr, $data:expr, $channel:expr) => {
        {
            let line = egui::plot::Line::new($data.lock().unwrap().plot_values($channel));
            let stroke = egui::Stroke::new(2.0, $color);
            $plot_ui.line(line.stroke(stroke));
        }
    };
}

pub struct RightSidebar {
    vars: Arc<RwLock<RasaVariables>>,
}

impl RightSidebar {
    pub fn new(program_vars: Arc<RwLock<RasaVariables>>) -> Self {
        Self {
            vars: program_vars,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let mut sidebar_text = String::new();
        ui.vertical(|ui| {
            ui.label("Sidebar");
            ui.separator();

            ui.checkbox(&mut self.vars.write().unwrap().show_box, "Show Box");

            ui.add(egui::Slider::new(&mut self.vars.write().unwrap().look_behind, 0..=25).text("X-Range").integer());
            ui.add(egui::Slider::new(&mut self.vars.write().unwrap().skip, 1..=60).text("Skip").integer());
        });
    }
}


pub struct Plots {
    vars: Arc<RwLock<RasaVariables>>,
}

impl Plots {
    pub fn new(program_vars: Arc<RwLock<RasaVariables>>) -> Self {
        Self {
            vars: program_vars,
        }
    }

    pub fn show_measurements(&self, ui: &mut egui::Ui, measurements: &Arc<Mutex<MeasurementWindow>>) {
        let measurement_plot = Plot::new("measurements").allow_drag(false);
        measurement_plot.show(ui, |plot_ui| {
            add_plot_line!(plot_ui, egui::Color32::LIGHT_GREEN, measurements, 0);
            add_plot_line!(plot_ui, egui::Color32::LIGHT_RED, measurements, 1);

            let series: PlotPoints = PlotPoints::new(measurements.lock().unwrap().rectpoints.clone());
            if self.vars.read().unwrap().show_box {
                let poly = Polygon::new(series);
                plot_ui.polygon(poly);
            }
        });
    }

    pub fn show_rewards(&self, ui: &mut egui::Ui, measurements: &Arc<Mutex<MeasurementWindow>>) {
        let mut reward_plot = egui::plot::Plot::new("rewards").allow_drag(false);
        reward_plot = reward_plot.include_y(300.0);
        reward_plot = reward_plot.include_y(200.0);

        reward_plot.show(ui, |plot_ui| {
            add_plot_line!(plot_ui, egui::Color32::GOLD, measurements, 4);
        });
    }
}


pub struct MonitorApp {
    pub rasa: Arc<RwLock<RasaVariables>>,
    pub  measurements: Arc<Mutex<MeasurementWindow>>,
    pub  reward: Arc<Mutex<MeasurementWindow>>,
    pub  feedback: Vec<f64>,

    sidebar: RightSidebar,
    plots: Plots,
    show_box: bool,
}

impl MonitorApp {
    pub fn new(vars: &Arc<RwLock<RasaVariables>>) -> Self {
        let var_l = vars.read().unwrap();
        Self {
            rasa: Arc::clone(&vars),
            measurements: Arc::new(Mutex::new(MeasurementWindow::new(
                Arc::clone(&vars)
            ))),
            reward: Arc::new(Mutex::new(MeasurementWindow::new(
                Arc::clone(&vars)
            ))),
            feedback: Vec::new(),

            sidebar: RightSidebar::new(Arc::clone(&vars)),
            plots: Plots::new(Arc::clone(&vars)),

            show_box: var_l.show_box
        }
    }

    pub fn update_rect(&mut self) {
        //self.measurements.lock().unwrap().values
    }
}


impl eframe::App for MonitorApp {
    /// Called by the frame work to save state before shutdown.
    /// Note that you must enable the `persistence` feature for this to work.
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let side_panel_width = 200.0;
        egui::CentralPanel::default().show(ctx, |ui| {
            let total_height = ui.available_size().y;
            let button_ratio = 0.7;
            let label_ratio = 0.3;

            let button_height = total_height * button_ratio;
            let label_height = total_height * label_ratio;

            ui.allocate_ui(Vec2::new(ui.available_size().x - side_panel_width, button_height), |ui| {
                self.plots.show_measurements(ui, &self.measurements);
            });

            ui.allocate_ui(Vec2::new(ui.available_size().x - side_panel_width, label_height), |ui| {
                self.plots.show_rewards(ui, &self.measurements);
            });
        });

        egui::SidePanel::right("Sidebar").show(ctx, |mut ui| {
            self.sidebar.show(&mut ui)
        });

        // make it always repaint
        ctx.request_repaint();
    }
}