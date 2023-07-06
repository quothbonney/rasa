mod measurements;
mod monitor;

use crate::monitor::MonitorApp;
use crate::measurements::MeasurementWindow;
use eframe::egui;
use std::time::{Instant, Duration};
use std::io::BufReader;
use std::io::BufRead;
use std::collections::VecDeque;
use std::sync::*;
use serialport::*;
use std::thread;
use tracing::{error, info, warn};
use std::f64;
use std::any::type_name;
use clap::{arg, Parser};

macro_rules! add_measurement {
    ($monitor_ref:expr, $value:expr) => {
        {
            let value = $value;
            $monitor_ref
                .lock()
                .unwrap()
                .add(0, measurements::Measurement::new(value.0.clone(), value.1.clone()));
        }
    };
}

fn main() {

    let mut app = MonitorApp::new(20, 2);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = app.measurements.clone();
    let t_interval: u64 = 5;

    let index = 0i32;
    let port = "COM3";
    let baud_rate = 9600;
    let ports = serialport::available_ports().expect("No ports found!");
    println!("{:?}", ports);
    let port = serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open port");
    let reader = std::io::BufReader::new(port);

    thread::spawn(move || {
        //let stdin = std::io::stdin();
        let mut index: i32 = 0;
        let mut start = Instant::now();


        for line in reader.lines() {
            let vals: Option<(f64, f64)>;
            match line {
                Ok(line) => {
                    // Here you can parse the line as per your serialization format.
                    // Assuming it's a string of integers separated by spaces:
                    let numbers: Vec<i32> = line
                        .split_whitespace()
                        .filter_map(|num| num.parse::<i32>().ok())
                        .collect();
                    println!("{:?}", numbers);
                    vals = Some((index as f64 / 10.0, numbers[0] as f64));
                    thread::sleep(Duration::from_millis(1));
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                    continue;
                },
            }

            match vals {
                Some(value) => {
                    add_measurement!(monitor_ref, value);
                }
                _ => {
                    warn!("Could not read from input stream at index {}", index);
                }
            };
            index += 1;
        }
    });

    info!("Main thread started");
    eframe::run_native("Monitor app", native_options, Box::new(|_| Box::new(app)));
}
