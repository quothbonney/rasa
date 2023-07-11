mod measurements;
mod monitor;

use std::fs::{File, OpenOptions};
use crate::monitor::MonitorApp;
use crate::measurements::MeasurementWindow;
use eframe::egui;
use csv::Writer;
use std::time::{Instant, Duration};
use std::io::{BufReader, Write};
use std::io::BufRead;
use std::collections::VecDeque;
use std::sync::*;
use serialport::*;
use std::thread;
use tracing::{error, info, warn};
use std::f64;
use std::any::type_name;
use clap::{arg, Parser};
use rand::Rng;
use rand_distr::{Normal, Distribution};

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


struct OrnsteinUhlenbeck {
    theta: f64,
    mu: f64,
    sigma: f64,
    x: f64,
}

impl OrnsteinUhlenbeck {
    fn new(theta: f64, mu: f64, sigma: f64, x0: f64) -> Self {
        OrnsteinUhlenbeck {
            theta,
            mu,
            sigma,
            x: x0,
        }
    }

    fn step(&mut self, dt: f64) -> f64 {
        let mut rng = rand::thread_rng();
        let normal = Normal::new(0.0, (2.0 * self.theta * self.sigma * dt).sqrt()).unwrap();
        let dw = normal.sample(&mut rng);
        let dx = self.theta * (self.mu - self.x) * dt + self.sigma * dw;
        self.x += dx;
        self.x
    }
}

enum InputStreams {
    TestStream,
    PhotometryStream,
    OrnsteinStream
}

fn average_vec(vec: &Vec<Vec<f64>>) -> (f64, f64) {
    let (sum0, count0) = vec.iter()
        .filter_map(|inner| inner.get(0))  // get first element, if it exists
        .fold((0.0, 0), |(sum, count), &val| (sum + val, count + 1)); // sum values and count elements

    let (sum1, count1) = vec.iter()
        .filter_map(|inner| inner.get(1))  // get first element, if it exists
        .fold((0.0, 0), |(sum, count), &val| (sum + val, count + 1)); // sum values and count elements

    let av = (sum0 / (count0 as f64), (sum1 / (count1 as f64)));
    av
}


fn std_dev(data1: &[f64], data2: &[f64]) -> (f64, f64) {
    let n1 = data1.len() as f64;
    let mean1 = data1.iter().sum::<f64>() / n1;

    let variance1 = data1.iter()
        .map(|&x| (x - mean1).powi(2))
        .sum::<f64>() / n1;

    let std_dev1 = variance1.sqrt();

    let n2 = data2.len() as f64;
    let mean2 = data2.iter().sum::<f64>() / n2;

    let variance2 = data2.iter()
        .map(|&x| (x - mean2).powi(2))
        .sum::<f64>() / n2;

    let std_dev2 = variance2.sqrt();

    (std_dev1, std_dev2)
}


fn qualifies_for_stimulation(v0: &[f64], v1: &[f64], averages: (f64, f64), stddevs: (f64, f64), percentage: f64) -> bool {
    let sigma_level = 1.0;
    let target0 = averages.0 + (sigma_level * stddevs.0);
    let target1 = averages.1 + (sigma_level * stddevs.1);

    println!("{}, {}", target0, target1);
    let mut count = (0i32, 0i32);
    let mut total = (0i32, 0i32);

    for i in v0 {
        if i < &target0 { count.0 += 1;};
        total.0 += 1;
    }

    for j in v1 {
        if j < &target1 { count.1 += 1;};
        total.1 += 1;
    }

    println!("Counts: {}, {}", count.0, count.1);

    let percentage_to_target = ((count.0 as f64 / total.0 as f64), (count.1 as f64 / total.1 as f64));
    println!("{:?}", percentage_to_target);
    percentage_to_target.0 > percentage && percentage_to_target.0 > percentage

}

fn main() {
    let fpath = "./sample.csv";
    let writemode = false;
    let mut app = MonitorApp::new(10, 4);
    let native_options = eframe::NativeOptions::default();
    let monitor_ref = app.measurements.clone();
    let t_interval: u64 = 5;

    let port = "COM4";
    let baud_rate = 115200;
    let ports = serialport::available_ports().expect("No ports found!");
    println!("{:?}", ports);

    // Data read/write channel
    let (tx, rx) = mpsc::channel();
    let active_thread = InputStreams::OrnsteinStream;
    let mut writer: Writer<File>;

    //if writemode {
        writer = Writer::from_writer(
            OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(fpath)
                .unwrap()
        );
    //}
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
                    writer.write_record(&[
                        elapsed.to_string(),
                        y.to_string(),
                        (y*y).to_string(),
                    ]);
                    thread::sleep(Duration::from_micros(10));
                    ix += 1;
                }
            });
        }
        InputStreams::OrnsteinStream => {
            let mut zapper_timer = Instant::now();
            let vec_mutex = Mutex::new(Vec::new());
            let start = Instant::now();
            let mut process = OrnsteinUhlenbeck::new(0.1, 0.0, 0.1, 0.0);
            let dt = 0.01;
            let mut sec_start = Instant::now();
            let mut old_average = (0f64, 0f64);
            let mut old_std = (0f64, 0f64);

            let mut writeport = serialport::new("COM5", baud_rate)
                .timeout(Duration::from_millis(10))
                .open()
                .expect("Failed to open port");



            thread::spawn(move || {
                //let reader = std::io::BufReader::new(port);
                let mut ix = 1i32;
                loop {
                    let y: f64 = process.step(dt);
                    let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
                    let num = (
                        (elapsed, y),
                        (elapsed, y*y),
                        (elapsed - 1.0, old_average.0),
                        (elapsed -1.0, old_average.1)
                    );
                    //println!("{:?}", num);
                    tx.send(num).unwrap();
                    let mut vec = vec_mutex.lock().unwrap();
                    vec.push(vec![y, y*y]);
                    if sec_start.elapsed() > Duration::from_secs(1) {
                        sec_start = Instant::now();
                        let v0 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(0)).collect::<Vec<_>>();
                        let v1 = vec.clone().into_iter().filter_map(|v| v.into_iter().nth(0)).collect::<Vec<_>>();
                        old_average = average_vec(&vec);
                        old_std = std_dev(&v0,&v1);
                        let qft = qualifies_for_stimulation(&v1, &v0, old_average, old_std, 0.3);
                        println!("Qualifies for stimulation? {}", qft);
                        println!("Stddev {:?}", old_std);
                         if zapper_timer.elapsed() > Duration::from_secs(2) {
                            zapper_timer = Instant::now();
                            println!("Stimulation Threshold Reached");
                            writeport.write(&['s' as u8]);
                        }
                        vec.clear();
                    }
                    writer.write_record(&[
                        elapsed.to_string(),
                        y.to_string(),
                        (y*y).to_string(),
                    ]);
                    thread::sleep(Duration::from_micros(10));
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

                                writer.write_record(&[
                                    elapsed.to_string(),
                                    numbers[0].to_string(),
                                    numbers[1].to_string(),
                                    //numbers[2].to_string(),
                                ]).unwrap();

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