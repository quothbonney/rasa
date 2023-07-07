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
    ($monitor_ref:expr, $value:expr, $channel:expr) => {
        {
            let value = $value;
            $monitor_ref
                .lock()
                .unwrap()
                .add($channel, measurements::Measurement::new(value.0.clone(), value.1.clone()));
        }
    };
}


fn main() {

    let mut app = MonitorApp::new(4, 2);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = app.measurements.clone();
    let t_interval: u64 = 5;

    let port = "COM3";
    let baud_rate = 9600;
    let ports = serialport::available_ports().expect("No ports found!");
    println!("{:?}", ports);

    let shared_var: Arc<RwLock<((f64, f64), (f64, f64))>> = Arc::new(RwLock::new(((0.0, 0.0), (0.0, 0.0))));



    let (tx, rx) = mpsc::channel();

    let photometery_writer = thread::spawn(move || {
        let mut index = 1i32;
        let port = serialport::new(port, baud_rate)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");

        let reader = std::io::BufReader::new(port);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    // Here you can parse the line as per your serialization format.
                    // Assuming it's a string of integers separated by spaces:
                    let numbers: Vec<f64> = line
                        .split_whitespace()
                        .filter_map(|num| num.parse::<f64>().ok())
                        .collect::<Vec<f64>>();
                    //println!("{:?}", numbers);
                    if numbers.len() >= 2 {
                        let num = (
                            (index as f64 / 100.0, numbers[0] as f64),
                            (index as f64 / 100.0, numbers[1] as f64),
                        );
                         tx.send(num).unwrap();  // send processed value
                    }
                    index += 1;
                    //thread::sleep(Duration::from_millis(1));
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                    continue;
                }
            }
        }
    });


/*
    let test_writer = thread::spawn(move || {
        //let reader = std::io::BufReader::new(port);
        let mut ix = 1i32;
        loop {
            let y: f64 = match ix % 100 {
                0 => 5.0,
                _ => (ix as f64).sin()
            };
            let num = (
                (ix as f64 / 100.0, y),
                (ix as f64 / 100.0, y*y),
            );
            //println!("{:?}", num);
            tx.send(num).unwrap();
            thread::sleep(Duration::from_millis(1));
            ix += 1;
        }
    });

 */

    let reader = thread::spawn(move || {
        loop {
            let mut last_received = None;

            // Drain the channel and keep only the last received value
            while let Ok(val) = rx.try_recv() {
                last_received = Some(val);
            }

            if let Some(val) = last_received {
                // Handle the received value
                add_measurement!(monitor_ref, val.0, 0);
                add_measurement!(monitor_ref, val.1, 1);
            }

        }
    });

        /*
        match vals {
            Some(value) => {

            }
            _ => {
                warn!("Could not read from input stream at index {}", index);
            }
        };
        */
/*



    thread::spawn(move || {
        //let stdin = std::io::stdin();
        let mut index: i32 = 0;
        let mut start = Instant::now();
        let elapsed = start.elapsed();

            match vals {
                Some(value) => {
                    add_measurement!(monitor_ref, value.0, 0);
                    add_measurement!(monitor_ref, value.1, 1);
                }
                _ => {
                    warn!("Could not read from input stream at index {}", index);
                }
            };
            index += 1;
        }
    });
*/
    info!("Main thread started");
    eframe::run_native("Monitor app", native_options, Box::new(|_| Box::new(app)));
}
