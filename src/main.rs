mod measurements;
mod monitor;
mod stim;
mod ornstein;
mod util;
mod threadedchannel;

use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::{Path, PathBuf};
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
use tracing::field::debug;

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
    PhotometryStream,
    OrnsteinStream
}

fn config_subscriber() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

fn get_fpath() -> (String, String) {
    let mut file_number = 0;
    let mut file_path = format!("data/data{}.csv", file_number);
    let mut reward_path = format!("data/reward{}.csv", file_number);

    while Path::new(&file_path).exists() {
        file_number += 1;
        file_path = format!("data/data{}.csv", file_number);
        reward_path = format!("data/reward{}.csv", file_number);
    }

    (file_path, reward_path)
}

fn main() {
    config_subscriber();

    // Get next available filepath in pattern {data/data<num>.csv}
    let (file_path , reward_path) = get_fpath();
    let is_ttl = Arc::new(Mutex::new(false));
    let ttl_clone = is_ttl.clone();

    let port = "COM4";
    let baud_rate = 115200;

    let mut vis_app = MonitorApp::new(10, 4);
    let mut reward_app = MonitorApp::new(10, 1);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = vis_app.measurements.clone();

    // Used in the analysis and the visualize threads. Rust is very particular about variable ownership, this seems
    // To work as a solution
    let vis_monitor = Arc::clone(&monitor_ref);
    let ai_monitor = Arc::clone(&monitor_ref);

    /*
    let reward_ref = reward_app.measurements.clone();
    let reward_vis_monitor = Arc::clone(&reward_ref);
    let reward_ai_monitor = Arc::clone(&reward_ref);
     */

    let ports = serialport::available_ports().expect("No ports found!");
    info!("{:?}", ports);

    let model = CModule::load("models/traced_model2.pt");
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
                .open(file_path)
                .unwrap()
        );

    info!("{:?}", reward_path);

    let mut r_writer: Writer<File> = Writer::from_writer(
        OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(reward_path)
            .unwrap()
    );

    // Custom VecDeque channels. Can be read from and written to without explicit locking
    // Have size of 64. Designed so that when an element is added, another is popped. Pretty cool
    let (tx_deque0, rx_deque0) = deque_channel(64);
    let (tx_deque1, rx_deque1) = deque_channel(64);
    let (tx_time, rx_time) = deque_channel(64);

    thread::spawn(move || {

        let mut writeport = serialport::new("COM5", baud_rate)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");
        let mut zapper_timer = Instant::now();
        let mut ix: usize = 0;
        let mut sigma = 3.7;
        let max_sigma = 5.5;
        let sigma_inc = 0.05;

        let max_size = 1024;
        let mut vec_deque: VecDeque<f64> = VecDeque::with_capacity(max_size);
        for i in 1..=max_size+5{
            vec_deque.push_back(i as f64);

            if vec_deque.len() > max_size {
                let popped_element = vec_deque.pop_front();
            }
        }
        loop {
            let v0: Vec<f64> = rx_deque0.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();
            let v1: Vec<f64> = rx_deque1.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();
            let v2: Vec<f64> = rx_time.deque.lock().unwrap().clone().into_iter().map(|value| value as f64).collect();

            let min_val = v1.clone().into_iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
            let max_val = v1.clone().into_iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);

            let min_time = v2.first();
            let max_time = v2.last();

            match (min_time, max_time) {
                (Some(min), Some(max)) => {
                    ai_monitor.lock().unwrap().update_rect(vec![
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
            //println!("{:?}", input_vec);

            match umodel.forward_ts(&[input_data]) {
                Ok(output_data) => {
                    // The forward method was successful and returned a Tensor
                    //println!("Output data: ");
                    let tens1: Tensor = output_data.mean_dim(1, false, Kind::Float);
                    //tens1.print();
                    // PEAK DETECTION TENSOR
                    let tens2 = Tensor::of_slice(&[-0.1309, -0.0426, -0.0295,  0.1515,  0.1200, -0.3180,  0.1198,  0.0594]);

                    let distance = tens1.dist(&tens2);
                    //tens1.print();
                    let distance_scalar = 1.0/distance.double_value(&[]);
                    vec_deque.push_back(distance_scalar);
                    vec_deque.pop_front();
                    //tens1.print();
                    //debug!("Distance: {}", distance_scalar);
                    r_writer
                        .write_record(&[
                            min_time.unwrap_or(&0.0).to_string(),
                            max_time.unwrap_or(&0.0).to_string(),
                            distance_scalar.to_string()
                        ]).expect("Could not write to CSV output");

                    let stddev = std_dev_vec_deque(&vec_deque).unwrap();
                    let average = average_vec_deque(&vec_deque).unwrap();
                    let zscore = (distance_scalar - average) / stddev;

                    if distance_scalar > 300.0 {
                        if zapper_timer.elapsed() > Duration::from_secs(16) {
                            zapper_timer = Instant::now();
                            if sigma < max_sigma {
                                sigma += sigma_inc;
                            }
                            if *ttl_clone.lock().unwrap() {
                                info!("Stimulation received after peak with reward {} and z-score {}", distance_scalar, zscore);
                                writeport.write(&['s' as u8]);
                            }
                            else {
                                warn!("Stimulation cannot be administered. TTL bit not received.")
                            }
                        }
                        else {
                            info!("Cooldown - received reward {} and z-score {}", distance_scalar, zscore);
                        }
                    }
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

                    thread::sleep(Duration::from_micros(10));
                    ix += 1;
                }
            });
        }
        InputStreams::OrnsteinStream => {
            let mut ttl_guard = is_ttl.lock().unwrap();
            *ttl_guard = true;

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
            info!("Beginning Photometry stream on active thread");

            let mut ix = 1i32;
            let skip = 40;
            let mut process = OrnsteinUhlenbeck::new(0.5, 0.5, 0.1, 0.0);

            thread::spawn(move || {
                let mut index = 1i32;
                let readport = serialport::new(port, baud_rate)
                    .timeout(Duration::from_millis(10))
                    .open()
                    .expect("Failed to open port");

                let vec_mutex = Mutex::new(Vec::new());
                let reader = std::io::BufReader::new(readport);
                let start = Instant::now();
                let mut sec_start = Instant::now();
                let mut zapper_timer = Instant::now();
                let mut old_average = (0f64, 0f64);
                let mut old_std = (0f64, 0f64);
                let mut last_ttl = 1;

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
                                //let smudge_coefficient: f64 = process.step(0.01) * 300.0;
                                let y0 = numbers[0];// + smudge_coefficient;
                                let y1 = numbers[1];// + smudge_coefficient;
                                let y2 = if let Some(value) = numbers.get(2) {
                                    *value
                                } else {
                                    0.0
                                };
                                if y2 as i32 == 0 && last_ttl == 0 && *is_ttl.lock().unwrap() == false {
                                    let mut ttl_guard = is_ttl.lock().unwrap();
                                    *ttl_guard = true;
                                    info!("Received TTL Signal.");
                                } else {
                                    last_ttl = y2 as i32;
                                }

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
                                        y2.to_string()
                                    ]).expect("Could not write to CSV output");


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

    let copied_options = native_options.clone();
    info!("Main thread started");
    eframe::run_native("Photometry App", native_options.clone(), Box::new(|_| Box::new(vis_app)));

    //eframe::run_native("Reward app", copied_options.clone(), Box::new(|_| Box::new(reward_app)));
}