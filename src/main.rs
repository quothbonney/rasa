mod measurements;
mod monitor;
mod stim;
mod ornstein;
mod util;
mod threadedchannel;

use std::collections::VecDeque;
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
use tracing::{error, info, warn};
use clap::{arg, Parser};
use tch::{CModule, Tensor, Kind};

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

const STDDEV: u32 = 50;

enum InputStreams {
    TestStream,
    PhotometryStream,
    OrnsteinStream
}

fn normalize_array(v0_: &Vec<f64>, v1_: &Vec<f64>) -> (Vec<f64>, Vec<f64>) {
    let mut v0 = v0_.clone();
    let mut v1 = v1_.clone();

    let min_val = v0.clone().into_iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    let max_val = v1.clone().into_iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    let range_val = max_val - min_val;

    let sum0: f64 = v0.iter().sum();
    let average0 = sum0 as f64 / v0.len() as f64;
    let sum1: f64 = v1.iter().sum();
    let average1 = sum1 as f64 / v1.len() as f64;

    for num in v0.iter_mut() {
        *num = (*num - average0) / STDDEV as f64;
    }
    for num in v1.iter_mut() {
        // Subtract an extra 3 to seperate the channels for the RNN
        *num = ((*num - average1) / STDDEV as f64) - 3.0;
    }

    (v0, v1)
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

    let writemode = false;
    let mut app = MonitorApp::new(10, 4);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = app.measurements.clone();

    let port = "COM4";
    let baud_rate = 115200;
    let ports = serialport::available_ports().expect("No ports found!");
    println!("{:?}", ports);

    let model = CModule::load("traced_model.pt");
    let umodel: CModule;
    match model {
        Ok(m) => {info!("Loaded torch model successfully"); umodel = m},
        Err(m) => {error!("Unable to load torch model. Aborting..."); panic!() }
    }

    // Data read/write channel
    let (tx, rx) = mpsc::channel();
    let active_thread = InputStreams::OrnsteinStream;

    let mut writer: Writer<File> = Writer::from_writer(
            OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(fpath)
                .unwrap()
        );

    let (tx_deque0, rx_deque0) = deque_channel(64);
    let (tx_deque1, rx_deque1) = deque_channel(64);

    thread::spawn(move || {
        loop {
            let v0: Vec<f64> = rx_deque0.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();
            let v1: Vec<f64> = rx_deque1.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();
            let (mut input_vec, nv1) = normalize_array(&v0, &v1);
            input_vec.extend(nv1);
            let input_data = Tensor::of_slice(&input_vec).unsqueeze(0).unsqueeze(2).to_kind(Kind::Float);

            match umodel.forward_ts(&[input_data]) {
                Ok(output_data) => {
                    // The forward method was successful and returned a Tensor
                    println!("Output data: ");
                    let tens1: Tensor = output_data.mean_dim(1, false, Kind::Float);
                    let tens2 = Tensor::of_slice(&[0.22652599,  0.01613411,  0.19352987, -0.16627036,  0.45365506,
                        0.11038084,  0.22680497,  0.03352911]);

                    let distance = (&tens1 - &tens2).pow(&Tensor::of_slice(&[2])).sum(Kind::Float).sqrt();
                    println!("Reward: {}", 1/distance);
                    //output_data.print();
                }
                Err(e) => {
                    // The forward method failed and returned a TchError
                    print!("Error: {}", e);
                }
            }
        }
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

            thread::spawn(move || {
                //let reader = std::io::BufReader::new(port);
                let mut ix = 1i32;
                loop {
                    let y: f64 = process.step(dt) * 50.0;
                    let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
                    let num = (
                        (elapsed, y),
                        (elapsed, y*y),
                        (elapsed - 1.0, old_average.0),
                        (elapsed -1.0, old_average.1)
                    );
                    //println!("{:?}", num);
                    tx.send(num).unwrap();
                    tx_deque0.send(y as f32);
                    tx_deque1.send((y*y) as f32);

                    let mut vec = vec_mutex.lock().unwrap();
                    vec.push(vec![y, y*y]);
                    if sec_start.elapsed() > Duration::from_secs(1) {
                        sec_start = Instant::now();
                        let v0 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(0)).collect::<Vec<_>>();
                        let v1 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(1)).collect::<Vec<_>>();
                        let qft = qualifies_for_stimulation(&v0, &v1, old_average, old_std, 0.3);

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
                            y.to_string(),
                            (y * y).to_string(),
                        ]).expect("Could not write to CSV output");

                    thread::sleep(Duration::from_micros(1));
                    ix += 1;
                }
            });
        }
        InputStreams::PhotometryStream => {
            let vec_mutex = Mutex::new(Vec::new());

            thread::spawn(move || {
                let mut index = 1i32;
                let readport = serialport::new(port, baud_rate)
                    .timeout(Duration::from_millis(10))
                    .open()
                    .expect("Failed to open port");

                let mut writeport = serialport::new("COM5", baud_rate)
                    .timeout(Duration::from_millis(10))
                    .open()
                    .expect("Failed to open port");

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

                                tx.send(num).unwrap();  // send processed value
                                let mut vec = vec_mutex.lock().unwrap();
                                vec.push(numbers);
                                if sec_start.elapsed() > Duration::from_secs(1) {
                                    sec_start = Instant::now();
                                    let v0 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(0)).collect::<Vec<_>>();
                                    let v1 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(1)).collect::<Vec<_>>();
                                    old_average = average_vec(&vec);
                                    old_std = std_dev(&v0,&v1);
                                    let qft = qualifies_for_stimulation(&v0, &v1, old_average, old_std, 0.3);
                                    println!("Qualifies for stimulation? {}", qft);

                                    println!("STDDEV at time {}: {:?}", elapsed, old_std);
                                    if qft && zapper_timer.elapsed() > Duration::from_secs(16){
                                        zapper_timer = Instant::now();
                                        println!("Stimulation Threshold Reached")
                                        //writeport.write(&['s' as u8])
                                    }
                                    //println!("Stddev {:?}", old_std);
                                    vec.clear();
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