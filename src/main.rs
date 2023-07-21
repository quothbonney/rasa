mod measurements;
mod monitor;
mod stim;
mod util;
mod threadedchannel;
mod structs;

use winit::window::Icon;
use winit::window::WindowBuilder;
use std::cmp::min;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::{Path, PathBuf};
use std::fs::{File, OpenOptions};
use crate::monitor::MonitorApp;
use crate::threadedchannel::deque_channel;
use crate::measurements::MeasurementWindow;
use streams::ornstein::OrnsteinUhlenbeck;
use crate::stim::*;
use crate::util::*;
use eframe::egui;
use csv::Writer;
use std::time::{Duration, Instant};
use std::io::{BufRead, BufReader, Write};
use std::sync::*;
use serialport::*;
use std::{sync, thread};
use tracing::{debug, error, info, warn};
use tch::{CModule, IndexOp, Kind, Tensor};
use tracing::field::debug;
//use clap::{Arg, App, SubCommand};
use std::str::FromStr;


mod streams {
    pub mod teststream;
    pub mod ornstein;
    pub mod photometry;
    pub mod instantreplay;
}

use streams::*;

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
    PhotometryStream,//(String, String),
    OrnsteinStream,
    InstantReplayStream(String)
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

    let program_vars = Arc::new(RwLock::new(structs::RasaVariables {
        show_box: true,

        look_behind: 4,
        skip: 30,
        channels:5,
    }));

    //println!("Got here");
    let mut vis_app = monitor::MonitorApp::new(&program_vars);
    //println!("Got here");
    //let mut reward_app = MonitorApp::new(10, 1);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = vis_app.measurements.clone();

    // Used in the analysis and the visualize threads. Rust is very particular about variable ownership, this seems
    // To work as a solution
    let vis_monitor = Arc::clone(&monitor_ref);
    let ai_monitor = Arc::clone(&monitor_ref);

    let ports = available_ports().expect("No ports found!");
    info!("{:?}", ports);

    let umodel : CModule;
    match CModule::load("models/traced_model2.pt") {
        Ok(m) => {info!("Loaded torch model successfully"); umodel = m},
        Err(m) => {error!("Unable to load torch model. Aborting..."); panic!() }
    }

    // Data read/write channel
    let active_thread = InputStreams::PhotometryStream;//InstantReplayStream(String::from("data/data85.csv"));

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

    let (tx, rx) = mpsc::channel();
    let (tx_reward, rx_reward) = mpsc::channel();
    // Custom VecDeque channels. Can be read from and written to without explicit locking
    // Have size of 64. Designed so that when an element is added, another is popped. Pretty cool
    let (tx_deque0, rx_deque0) = deque_channel(64);
    let (tx_deque1, rx_deque1) = deque_channel(64);
    let (tx_time, rx_time) = deque_channel(64);

    thread::spawn(move || {
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
                    let avg_time = max_time.unwrap_or(&0.0) / 1.0;
                    tx_reward.send((avg_time, distance_scalar));

                    if distance_scalar > 300.0 {
                        if zapper_timer.elapsed() > Duration::from_secs(16) {
                            zapper_timer = Instant::now();
                            if sigma < max_sigma {
                                sigma += sigma_inc;
                            }
                            if *ttl_clone.lock().unwrap() {
                                info!("Stimulation received after peak with reward {} and z-score {}", distance_scalar, zscore);
                                //writeport.write(&['s' as u8]);
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
                    error!("Error: {:?}", e);
                }
            }
            thread::sleep(Duration::from_millis(10));
            if ix == 0 { info!("Begun analysis thread successfuly") }
            ix += 1;
        }
    });

    match active_thread {
        InputStreams::InstantReplayStream(file) => {
            thread::spawn(move || {
                streams::instantreplay::start_instant_replay(file, tx, &tx_deque0, &tx_deque1, &tx_time, writer, &program_vars);
            });
        }
        InputStreams::TestStream => { // Stream for Sin + spike data
            thread::spawn(move || {
                streams::teststream::start_test_stream(tx);
            });
        }
        InputStreams::OrnsteinStream => {
            thread::spawn(move || {
                streams::ornstein::start_ornstein_stream(tx, &tx_deque0, &tx_deque1, &tx_time, writer, is_ttl);
            });
        }
        InputStreams::PhotometryStream => {
            thread::spawn(move || {
                // TODO: Make sure is_ttl is working
                streams::photometry::start_photometry_stream(&String::from("COM4"), &String::from("COM5"), tx, &tx_deque0, &tx_deque1, &tx_time, writer, is_ttl);
            });
        }
    }

    let reader = thread::spawn(move || {
        loop {
            let mut last_received = None;
            let mut last_reward = None;

            // Drain the channel and keep only the last received value
            while let Ok(val) = rx.try_recv() {
                last_received = Some(val);
            }

            while let Ok(val) = rx_reward.try_recv() {
                last_reward = Some(val);
            }

            if let Some(val) = last_received {
                // Handle the received value
                add_measurement!(*vis_monitor, val[0], 0);
                add_measurement!(*vis_monitor, val[1], 1);
                add_measurement!(*vis_monitor, val[2], 2);
                add_measurement!(*vis_monitor, val[3], 3);
            }

            if let Some(val_r) = last_reward {
                add_measurement!(*vis_monitor, val_r, 4);
            }
        }
    });

    let copied_options = native_options.clone();
    info!("Main thread started");
    eframe::run_native("Photometry App", native_options.clone(), Box::new(|_| Box::new(vis_app)));

    //eframe::run_native("Reward app", copied_options.clone(), Box::new(|_| Box::new(reward_app)));
}