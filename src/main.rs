mod measurements;

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

pub struct MonitorApp {
    include_y: Vec<f64>,
    measurements: Arc<Mutex<MeasurementWindow>>,
}

impl MonitorApp {
    fn new(look_behind: usize, channels: usize) -> Self {
        Self {
            measurements: Arc::new(Mutex::new(MeasurementWindow::new_with_look_behind(
                look_behind,
                channels
            ))),
            include_y: Vec::new(),
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
                plot_ui.line(egui::plot::Line::new(
                    self.measurements.lock().unwrap().plot_values(0),
                )
                    .stroke(egui::Stroke::new(2.0, egui::Color32::LIGHT_RED))
                );

                plot_ui.line(egui::plot::Line::new(
                    self.measurements.lock().unwrap().plot_values(1),
                )
                    .stroke(egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE))
                );
            });
        });
        // make it always repaint. TODO: can we slow down here?
        ctx.request_repaint();
    }
}

trait DataInputStream {
    fn index(&self, ix: usize) -> Option<(f64, f64)>;
}

struct SineInput;

struct 

impl DataInputStream for SineInput {
    fn index(&self, ix: usize) -> Option<(f64, f64)> {
        let x = (ix as f64 / 10.0);
        let y = (ix as f64 / 10.0).sin();

        Some((x, y))
    }
}


/// Simple program to greet a person
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Look-behind window size
    #[clap(short, long, default_value_t = 1000)]
    window_size: usize,

    #[clap(short, long)]
    include_y: Vec<f64>,
}

fn main() {
    /*
    let port = "COM3";
    let baud_rate = 9600;
    let ports = serialport::available_ports().expect("No ports found!");
    let mut port = serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open port");

    let mut reader = BufReader::new(port);
    let mut my_str = String::new();
    loop {
        let pstring = reader.read_line(&mut my_str);
        let payload = match pstring {
            Ok(_) => {
                let value = my_str
                    .trim()
                    .split_whitespace()
                    .map(|s| s.parse().unwrap())
                    .collect::<Vec<_>>();
                value
            }
            Err(e) => {
                eprintln!("Error reading line: {}", e);
                Vec::<i32>::new()
            }
        };
        // Process the payload or do something with it

        // Clear the string for the next iteration
        my_str.clear();
    }
    */

    let mut app = MonitorApp::new(20, 2);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = app.measurements.clone();
    let t_interval: u64 = 5;

    const sis: SineInput = SineInput;

    thread::spawn(move || {
       //let stdin = std::io::stdin();
        let mut count: i32 = 0;
        let mut start = Instant::now();
        for i in 0.. {
            // Load points from sinusoid data input stream
            match sis.index(i) {
                Some(value) => {
                    monitor_ref
                        .lock()
                        .unwrap()
                        .add(0, measurements::Measurement::new(value.0.clone(), value.1.clone()));

                    monitor_ref
                        .lock()
                        .unwrap()
                        .add(1, measurements::Measurement::new((value.0).clone() , (value.1.clone())*(value.1.clone())));
                }
                _ => {
                    warn!("Could not read from {} at index {}", type_name::<SineInput>(), i);
                }
            };

            if start.elapsed() < Duration::from_secs(t_interval) {
                count += 1;
            } else {
                println!("Points per second: {}", count / t_interval as i32);
                count = 0;
                start = Instant::now();
            }
            thread::sleep(Duration::from_millis(1))
        }

    });

    info!("Main thread started");
    eframe::run_native("Monitor app", native_options, Box::new(|_| Box::new(app)));
}
