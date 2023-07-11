use crate::measurements::MeasurementWindow;
use eframe::egui;
use std::time::{Instant, Duration};
use std::io::BufReader;
use std::io::BufRead;
use std::sync::*;
use serialport::*;
use std::thread;
use tracing::{error, info, warn};
use std::f64;
use std::any::type_name;
use clap::{arg, Parser};

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
    pub  feedback: Vec<f64>,
}

impl MonitorApp {
    pub fn new(look_behind: usize, channels: usize) -> Self {
        Self {
            measurements: Arc::new(Mutex::new(MeasurementWindow::new_with_look_behind(
                look_behind,
                channels
            ))),
            include_y: Vec::new(),
            feedback: Vec::new(),
        }
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
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut plot = egui::plot::Plot::new("measurements");
            for y in self.include_y.iter() {
                plot = plot.include_y(*y);
            }

            plot.show(ui, |plot_ui| {
                add_plot_line!(plot_ui, egui::Color32::LIGHT_GREEN, self.measurements, 0);
                add_plot_line!(plot_ui, egui::Color32::LIGHT_RED, self.measurements, 1);
                add_plot_line!(plot_ui, egui::Color32::LIGHT_BLUE, self.measurements, 2);
                add_plot_line!(plot_ui, egui::Color32::LIGHT_BLUE, self.measurements, 3);

                // Add vertical line. Replace 'x_value' with the x value where you want the line.
                // You should also replace 'y_min' and 'y_max' with the minimum and maximum y values of your plot.
                //let vertical_line = vec![(2.0, -5.0), (2.0, 5.0)];
                //plot_ui.line(egui::plot::Line::new(egui::plot::Values::from_values_iter(vertical_line.into_iter())).color(egui::Color32::BLACK));
            });
        });
        // make it always repaint. TODO: can we slow down here?
        ctx.request_repaint();
    }
}