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

macro_rules! add_plot_line {
    ($plot_ui:expr, $color:expr, $data:expr, $channel:expr) => {
        {
            let line = egui::plot::Line::new($data.lock().unwrap().plot_values($channel));
            let stroke = egui::Stroke::new(2.0, $color);
            $plot_ui.line(line.stroke(stroke));
        }
    };
}


pub struct MonitorApp {
    pub include_y: Vec<f64>,
    pub  measurements: Arc<Mutex<MeasurementWindow>>,
    pub  reward: Arc<Mutex<MeasurementWindow>>,
    pub  feedback: Vec<f64>,

    show_box: bool,
}

impl MonitorApp {
    pub fn new(look_behind: usize, channels: usize) -> Self {
        Self {
            measurements: Arc::new(Mutex::new(MeasurementWindow::new_with_look_behind(
                look_behind,
                channels
            ))),
            reward: Arc::new(Mutex::new(MeasurementWindow::new_with_look_behind(
                look_behind,
                channels
            ))),
            include_y: Vec::new(),
            feedback: Vec::new(),
            show_box: true,
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
        let mut sidebar_text = String::new();

        let side_panel_width = 200.0;
        egui::CentralPanel::default().show(ctx, |ui| {
            let total_height = ui.available_size().y;
            let button_ratio = 0.7;
            let label_ratio = 0.3;

            let button_height = total_height * button_ratio;
            let label_height = total_height * label_ratio;

                ui.allocate_ui(Vec2::new(ui.available_size().x - side_panel_width, button_height), |ui| {
                    let mut plot = egui::plot::Plot::new("measurements").allow_drag(false);
                    for y in self.include_y.iter() {
                        plot = plot.include_y(*y);
                    }

                    plot.show(ui, |plot_ui| {
                        add_plot_line!(plot_ui, egui::Color32::LIGHT_GREEN, self.measurements, 0);
                        add_plot_line!(plot_ui, egui::Color32::LIGHT_RED, self.measurements, 1);
                        //add_plot_line!(plot_ui, egui::Color32::LIGHT_BLUE, self.measurements, 2);
                        //add_plot_line!(plot_ui, egui::Color32::LIGHT_BLUE, self.measurements, 3);

                        let series: PlotPoints = PlotPoints::new(self.measurements.lock().unwrap().rectpoints.clone());
                        if self.show_box {
                            let poly = Polygon::new(series);
                            plot_ui.polygon(poly);
                        }
                    });
                });

            ui.allocate_ui(Vec2::new(ui.available_size().x - side_panel_width, label_height), |ui| {
                let mut reward_plot = egui::plot::Plot::new("rewards").allow_drag(false);
                reward_plot = reward_plot.include_y(300.0);
                reward_plot = reward_plot.include_y(200.0);

                reward_plot.show(ui, |plot_ui| {
                    add_plot_line!(plot_ui, egui::Color32::GOLD, self.measurements, 4);
                });
            });
        });

        // Create a side panel.
        egui::SidePanel::right("Sidebar").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label("Sidebar");
                ui.separator();

                if ui.button("Show Box").clicked() {
                    self.show_box = !self.show_box;
                }
                if ui.button("Button 2").clicked() {
                    sidebar_text = "Button 2 clicked".to_string();
                }
                if ui.button("Button 3").clicked() {
                    sidebar_text = "Button 3 clicked".to_string();
                }
            });
        });
        // make it always repaint. TODO: can we slow down here?
        ctx.request_repaint();
    }
}