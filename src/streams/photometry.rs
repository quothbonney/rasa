use std::ops::Bound;
use rand::Rng;
use rand_distr::{Normal, Distribution};
use std::time::{Instant, Duration};
use std::thread;
use tracing::{debug, error, info, warn};
use std::sync::{Mutex, Arc};
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};
use csv::Writer;
use std::fs::{File, OpenOptions};
use std::io::BufRead;


use crate::threadedchannel::{BoundedSender, deque_channel};
use crate::util::*;

pub fn start_photometry_stream(inport: &String, tx: Sender<[(f64, f64); 4]>, tx_deque0: &BoundedSender, tx_deque1: &BoundedSender, tx_time: &BoundedSender, mut writer: Writer<File>,is_ttl: Arc<Mutex<bool>>) {
    info!("Beginning Photometry stream on active thread");
    //let port = "COM3";
    let baud_rate = 115200;

    let mut ix = 1i32;
    let skip = 40;
    let mut index = 1i32;
    let readport = serialport::new(inport, baud_rate)
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
                    let num = [
                        (elapsed, y0 as f64),
                        (elapsed, y1 as f64),
                        (elapsed - 1.0, old_average.0),
                        (elapsed - 1.0, old_average.1),
                    ];

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
}