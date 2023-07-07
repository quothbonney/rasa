mod measurements;
mod monitor;

use crate::monitor::MonitorApp;
use crate::measurements::MeasurementWindow;
use eframe::egui;
use std::time::{Instant, Duration};
use std::io::{BufReader, Read};
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

enum InputStreams {
    TestStream,
    PhotometryStream
}

fn vector_average(vec: &Vec<f64, f64>) -> (f64, f64) {
    let (sum0, count0) = vec.iter()
        .filter_map(|inner| inner.get(0))  // get first element, if it exists
        .fold((0.0, 0), |(sum, count), &val| (sum + val, count + 1)); // sum values and count elements

    let (sum1, count1) = vec.iter()
        .filter_map(|inner| inner.get(1))  // get first element, if it exists
        .fold((0.0, 0), |(sum, count), &val| (sum + val, count + 1)); // sum values and count elements
    let av = (sum0 / (count0 as f64), (sum1 / (count1 as f64)));
    av
}

fn io_ports()

fn main() {

    let mut app = MonitorApp::new(10, 4);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = app.measurements.clone();
    let t_interval: u64 = 5;

    let portname = "COM4";
    let baud_rate = 115200;
    let ports = serialport::available_ports().expect("No ports found!");
    println!("{:?}", ports);

    // Data read/write channel
    let (tx, rx) = mpsc::channel();
    let active_thread = InputStreams::PhotometryStream;



    let write_to_port = |port: &mut Box<dyn SerialPort>, character: u8| {
        let bytes_written = port.write(&[character]);
        match bytes_written {
            Ok(bytes) => println!("Wrote {} bytes", bytes),
            Err(e) => eprintln!("Failed to write to port: {}", e),
        }
    };

    match active_thread {
        InputStreams::TestStream => {
            let start = Instant::now();
            thread::spawn(move || {
                //let reader = std::io::BufReader::new(port);
                let mut ix = 1i32;
                loop {
                    let y: f64 = match ix % 100 {
                        0 => 5.0,
                        _ => (ix as f64).sin()
                    };
                    let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
                    let num = (
                        (elapsed, y),
                        (elapsed, y*y),
                        (elapsed, 0.0),
                        (elapsed, 0.0)
                    );
                    //println!("{:?}", num);
                    tx.send(num).unwrap();
                    thread::sleep(Duration::from_micros(10));
                    ix += 1;
                }
            });
        }
        InputStreams::PhotometryStream => {
            let output_port = serialport::new("COM4", baud_rate)
                .timeout(Duration::from_millis(10))
                .open()
                .expect("Failed to open port");

            let mut input_port = serialport::new("COM5", baud_rate)
                .timeout(Duration::from_millis(10))
                .open()
                .expect("Failed to open port");

            let vec_mutex = Mutex::new(Vec::new());
            thread::spawn(move || {
                let mut index = 1i32;

                let reader = std::io::BufReader::new(output_port);
                let start = Instant::now();
                let mut sec_start = Instant::now();
                let mut old_average = (0f64, 0f64);

                for line in reader.lines() {
                    //println!("Size of reader: {:?}", &reader.().lines().size_hint());
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
                                let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
                                let num = (
                                    (elapsed, numbers[0] as f64),
                                    (elapsed, numbers[1] as f64),
                                    (elapsed - 1.0, old_average.0),
                                    (elapsed - 1.0, old_average.1),
                                );
                                tx.send(num).unwrap();  // send processed value
                                let mut vec = vec_mutex.lock().unwrap();
                                vec.push(numbers);
                                if sec_start.elapsed() > Duration::from_secs(1) {
                                    sec_start = Instant::now();
                                    old_average = vector_average(&vec.unwrap());
                                    println!("Average: {:?}", old_average);
                                    vec.clear();
                                    //write_to_port(&mut input_port, b'A');
                                }
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
        }
    }

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
                add_measurement!(monitor_ref, val.2, 2);
                add_measurement!(monitor_ref, val.3, 3);
            }

        }
    });

    info!("Main thread started");
    eframe::run_native("Monitor app", native_options, Box::new(|_| Box::new(app)));
}
