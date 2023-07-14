mod measurements;
mod monitor;
mod stim;
mod ornstein;
mod util;
mod threadedchannel;

use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::Path;
use std::fs::{File, OpenOptions};
use crate::monitor::MonitorApp;
use crate::threadedchannel::{deque_channel};
use crate::measurements::MeasurementWindow;
use crate::ornstein::OrnsteinUhlenbeck;
use crate::stim::*;
use crate::util::*;
use eframe::egui;
use csv::Writer;
use std::time::{Instant, Duration};
use std::io::{Write, BufRead};
use std::sync::*;
use serialport::*;
use std::thread;
use tracing::{error, info, debug, warn};
use clap::{arg, Parser};
use tch::{IndexOp, CModule, Tensor, Kind};

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

const STDDEV: f64 = 1.0;

enum InputStreams {
    TestStream,
    PhotometryStream,
    OrnsteinStream
}

fn normalize_array(lower_: &Vec<f64>, high_: &Vec<f64>) -> (Vec<f64>, Vec<f64>) {
    let mut v0 = lower_.clone();
    let mut v1 = high_.clone();
    let min_val0 = v0.iter().chain(v0.iter()).cloned().fold(f64::INFINITY, f64::min);
    let max_val0 = v0.iter().chain(v0.iter()).cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_val1 = v1.iter().chain(v1.iter()).cloned().fold(f64::INFINITY, f64::min);
    let max_val1 = v1.iter().chain(v1.iter()).cloned().fold(f64::NEG_INFINITY, f64::max);

    let min_val = min_val0.min(min_val1);
    let max_val = max_val0.max(max_val1);

    let mut normalized_arr: (Vec<f64>, Vec<f64>) = (
        v0.iter().map(|&x| (x - min_val) / STDDEV).collect(),
        v1.iter().map(|&x| (x - min_val) / STDDEV).collect(),
    );

    let avg0: f64 = normalized_arr.0.iter().sum::<f64>() / normalized_arr.0.len() as f64;
    let avg1: f64 = normalized_arr.1.iter().sum::<f64>() / normalized_arr.1.len() as f64;

    normalized_arr.0.iter_mut().for_each(|x| *x -= avg0);
    normalized_arr.1.iter_mut().for_each(|x| *x -= (avg1 + 3.0));
    //debug!("{:?}", normalized_arr);

    normalized_arr
}

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let fpath = Path::new("./sample.csv");
    if fpath.exists() {
        warn!("Logging location {} exists, persisting...", fpath.to_str().expect("UNKNOWN"));
    } else {
        info!("Beginning logs in {}", fpath.to_str().expect("UNKNOWN"));
    }

    let r_points: Mutex<Vec<[f64; 2]>> = Mutex::new(vec![[0.0; 2]; 4]);

    let writemode = false;
    let mut app = MonitorApp::new(10, 4);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = app.measurements.clone();
    let vis_monitor = Arc::clone(&monitor_ref);
    let box_monitor = Arc::clone(&monitor_ref);

    let port = "COM3";
    let baud_rate = 115200;
    let ports = serialport::available_ports().expect("No ports found!");
    println!("{:?}", ports);

    let model = CModule::load("traced_model2.pt");
    let umodel: CModule;
    match model {
        Ok(m) => {info!("Loaded torch model successfully"); umodel = m},
        Err(m) => {error!("Unable to load torch model. Aborting..."); panic!() }
    }

    // Data read/write channel
    let (tx, rx) = mpsc::channel();
    let active_thread = InputStreams::PhotometryStream;

    let mut writer: Writer<File> = Writer::from_writer(
            OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(fpath)
                .unwrap()
        );

    // Custom VecDeque channels. Can be read from and written to without explicit locking
    // Have size of 64. Designed so that when an element is added, another is popped. Pretty cool
    let (tx_deque0, rx_deque0) = deque_channel(64);
    let (tx_deque1, rx_deque1) = deque_channel(64);
    let (tx_time, rx_time) = deque_channel(64);

    thread::spawn(move || {
        let mut ix: usize = 0;
        loop {
            let v0: Vec<f64> = rx_deque0.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();
            let v1: Vec<f64> = rx_deque1.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();
            let v2: Vec<f64> = rx_time.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();

            //debug!("{:?}", v0);

            let min_val = v1.clone().into_iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
            let max_val = v1.clone().into_iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);

            let min_time = v2.first();
            let max_time = v2.last();
            //println!("{:?}, {:?}", min_val, max_val);

            match (min_time, max_time) {
                (Some(min), Some(max)) => {
                    box_monitor.lock().unwrap().update_rect(vec![
                        [*max, max_val],
                        [*max, min_val],
                        [*min, min_val],
                        [*min, max_val],
                    ]);
                },
                _ => {
                    continue;
                }
            }

            let (mut input_vec, nv1) = normalize_array(&v0, &v1);
            input_vec.extend(nv1);
            let input_data = Tensor::of_slice(&input_vec).unsqueeze(0).unsqueeze(2).to_kind(Kind::Float);

            //debug!("{:?}", input_vec);
            match umodel.forward_ts(&[input_data]) {
                Ok(output_data) => {
                    // The forward method was successful and returned a Tensor
                    //println!("Output data: ");
                    let tens1: Tensor = output_data.mean_dim(1, false, Kind::Float);
                    tens1.print();
                    // PEAK DETECTION TENSOR
                    let tens2 = Tensor::of_slice(&[-0.4445,  0.0094,  0.3916,  0.0283, -0.0047,  0.0379, -0.1839, -0.0370]);

                    let distance = tens1.dist(&tens2);
                    let distance_scalar = distance.double_value(&[]);
                    debug!("Distance: {}", 1.0/distance_scalar);
                }
                Err(e) => {
                    // The forward method failed and returned a TchError
                    error!("Error: {}", e);
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        ix += 1;
    });

    match active_thread {
        InputStreams::TestStream => { // Stream for Sin + spike data
            let start = Instant::now();
            thread::spawn(move || {
                let mut ix = 1i32;
                loop {
                    let y: f64 = match ix % 100 {
                        0 => 2.0,
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
                    if writemode {
                        writer.write_record(&[
                            elapsed.to_string(),
                            y.to_string(),
                            (y * y).to_string(),
                        ]);
                    }

                    thread::sleep(Duration::from_micros(10));
                    ix += 1;
                }
            });
        }
        InputStreams::OrnsteinStream => {
            info!("Beginning Ornstein stream on active thread");
            let mut zapper_timer = Instant::now();
            let vec_mutex = Mutex::new(Vec::new());
            let start = Instant::now();
            let mut process = OrnsteinUhlenbeck::new(0.5, 0.5, 0.1, 0.0);
            let dt = 0.01;
            let mut sec_start = Instant::now();
            let mut old_average = (0f64, 0f64);
            let mut old_std = (0f64, 0f64);
            let y0: f64;
            let y1: f64;

            thread::spawn(move || {
                //let reader = std::io::BufReader::new(port);
                let mut ix = 1i32;
                loop {
                    let y0: f64 = process.step(dt) * 50.0;
                    let y1 = y0 - 10.0;
                    let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
                    let num = (
                        (elapsed, y0),
                        (elapsed, y1),
                        (elapsed - 1.0, old_average.0),
                        (elapsed -1.0, old_average.1)
                    );
                    tx.send(num).unwrap();
                    tx_deque0.send(y0 as f32);
                    tx_deque1.send(y1 as f32);
                    tx_time.send(elapsed as f32);

                    let mut vec = vec_mutex.lock().unwrap();
                    vec.push(vec![y0, y1]);
                    if sec_start.elapsed() > Duration::from_secs(1) {
                        sec_start = Instant::now();
                        let v0 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(0)).collect::<Vec<_>>();
                        let v1 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(1)).collect::<Vec<_>>();
                        //let qft = qualifies_for_stimulation(&v0, &v1, old_average, old_std, 0.3);

                        old_average = average_vec(&vec);
                        old_std = std_dev(&v0,&v1);
                        //println!("Qualifies for stimulation? {}", qft);
                        //println!("Stddev {:?}", old_std);
                        vec.clear();
                    }

                    let current_time = SystemTime::now();
                    let unix_timestamp_ms = current_time.duration_since(UNIX_EPOCH)
                        .expect("Time went backwards")
                        .as_millis();

                    writer
                        .write_record(&[
                            elapsed.to_string(),
                            unix_timestamp_ms.to_string(),
                            y0.to_string(),
                            y1.to_string(),
                        ]).expect("Could not write to CSV output");

                    thread::sleep(Duration::from_micros(1));
                    ix += 1;
                }
            });
        }
        InputStreams::PhotometryStream => {
            let mut ix = 1i32;
            let skip = 40;

            thread::spawn(move || {
                let mut index = 1i32;
                let readport = serialport::new(port, baud_rate)
                    .timeout(Duration::from_millis(10))
                    .open()
                    .expect("Failed to open port");
                /*
                let mut writeport = serialport::new("COM5", baud_rate)
                    .timeout(Duration::from_millis(10))
                    .open()
                    .expect("Failed to open port");

                 */

                let vec_mutex = Mutex::new(Vec::new());
                let reader = std::io::BufReader::new(readport);
                let start = Instant::now();
                let mut sec_start = Instant::now();
                let mut zapper_timer = Instant::now();
                let mut old_average = (0f64, 0f64);
                let mut old_std = (0f64, 0f64);

                for line in reader.lines() {
                    //println!("Size of reader: {:?}", &reader.().lines().size_hint());
                    //println!("{:?}", line);
                    match line {
                        Ok(line) => {
                            //println!("{}", &line);
                            // Here you can parse the line as per your serialization format.
                            // Assuming it's a string of integers separated by spaces:
                            let numbers: Vec<f64> = line
                                .split_whitespace()
                                .filter_map(|num| num.parse::<f64>().ok())
                                .collect::<Vec<f64>>();

                            //println!("{:?}", numbers);
                            if numbers.len() >= 2 {
                                let y0 = numbers[0];
                                let y1 = numbers[1];
                                let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
                                let num = (
                                    (elapsed, y0 as f64),
                                    (elapsed, y1 as f64),
                                    (elapsed - 1.0, old_average.0),
                                    (elapsed - 1.0, old_average.1),
                                );

                                tx.send(num).unwrap();
                                if ix % skip == 0 {
                                    tx_deque0.send(y0 as f32);
                                    tx_deque1.send(y1 as f32);
                                    tx_time.send(elapsed as f32);
                                }
/*
                                writer
                                    .expect("Writemode enabled, but no configured writer")
                                    .write_record(&[
                                    elapsed.to_string(),
                                    numbers[0].to_string(),
                                    numbers[1].to_string(),
                                    //numbers[2].to_string(),
                                ]).unwrap();

 */

                                let mut vec = vec_mutex.lock().unwrap();
                                vec.push(vec![y0, y1]);
                                if sec_start.elapsed() > Duration::from_secs(1) {
                                    sec_start = Instant::now();
                                    let v0 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(0)).collect::<Vec<_>>();
                                    let v1 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(1)).collect::<Vec<_>>();
                                    old_average = average_vec(&vec);
                                    old_std = std_dev(&v0,&v1);
                                    //let qft = qualifies_for_stimulation(&v0, &v1, old_average, old_std, 0.3);
                                   /*
                                    if qft && zapper_timer.elapsed() > Duration::from_secs(16){
                                        zapper_timer = Instant::now();
                                        println!("Stimulation Threshold Reached")
                                        //writeport.write(&['s' as u8])
                                    }
                                    //println!("Stddev {:?}", old_std);

                                    */
                                    vec.clear();
                                }
                            }
                            ix += 1;
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
                add_measurement!(*vis_monitor, val.0, 0);
                add_measurement!(*vis_monitor, val.1, 1);
                add_measurement!(monitor_ref, val.2, 2);
                add_measurement!(monitor_ref, val.3, 3);
            }
        }
    });

    info!("Main thread started");
    eframe::run_native("Monitor app", native_options, Box::new(|_| Box::new(app)));
}