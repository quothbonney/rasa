use std::sync::{Mutex, Arc};
use std::sync::mpsc::Sender;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use spin_sleep::sleep;
use tracing::{debug, error, info, warn};
use csv::Writer;
use std::thread;
use std::time::{Instant, Duration};
use serde::Deserialize;

use crate::threadedchannel::{BoundedSender, deque_channel};
use crate::util::*;

use std::error::Error;
use std::io::Read;
use std::path::Path;

pub fn start_instant_replay(file: String, tx: Sender<[(f64, f64); 4]>, tx_deque0: &BoundedSender, tx_deque1: &BoundedSender, tx_time: &BoundedSender, mut writer: Writer<File>,is_ttl: Arc<Mutex<bool>>) {
    let vec_mutex = Mutex::new(Vec::new());
    let path = Path::new(file);
    let file = File::open(&path);
    let mut sec_start = Instant::now();
    let start = Instant::now();
    let mut ix: usize = 0;
    let mut old_average = (0f64, 0f64);
    let mut old_std = (0f64, 0f64);

    // Create a CSV reader.
    let mut reader = csv::Reader::from_reader(file.unwrap());

    for result in reader.records() {
        // The iterator yields Result<StringRecord, Error>, so we check the error here.
        let record = result.unwrap();

        // Ensure there are exactly 5 columns
        if record.len() != 5 {
            error!("Expected 5 columns");
        }

        // Parse the columns into f64 values.
        let values: Result<Vec<f64>, _> = record.iter().map(|s| s.parse()).collect();

        // If parsing was successful, print the values. Otherwise, return the error.
        let  (mut y0, mut y1) = (0.0, 0.0);
        match values {
            Ok(values) => {
                y0 = values[2];
                y1 = values[3];
            },
            Err(err) => error!("Could not read line!"),
        }
        let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
        let num = [
            (elapsed, y0 as f64),
            (elapsed, y1 as f64),
            (elapsed, 0.0),
            (elapsed, 0.0),
        ];
        let skip = 40;
        tx.send(num).unwrap();
        if ix % skip == 0 {
            tx_deque0.send(y0 as f32);
            tx_deque1.send(y1 as f32);
            tx_time.send(elapsed as f32);
        }

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
        ix += 1;
        sleep(Duration::from_millis(1));
    }
    /*
    let mut rdr = csv::Reader::from_reader(std::io::stdin());
    match rdr {
        Ok(mut r) => {
            let mut ix: usize = 0;
            for result in r.deserialize() {
                // The iterator yields Result<StringRecord, Error>, so we check the
                // error here
                let record: Record = result.unwrap();
                match (
                    record.col3,
                    record.col4) {
                    (Some(y0), Some(y1)) => {
                        let elapsed = (start.elapsed().as_millis() as f64) / 1000.0;
                        //println!("{} {} {}", elapsed, y0, y1);
                        let num = [
                            (elapsed, y0 as f64),
                            (elapsed, y1 as f64),
                            (0.0, 0.0),
                            (0.0, 0.0),
                        ];
                        let skip = 4;
                        tx.send(num).unwrap();
                        if ix % skip == 0 {
                            tx_deque0.send(y0 as f32);
                            tx_deque1.send(y1 as f32);
                            tx_time.send(elapsed as f32);
                        }
                        let mut vec = vec_mutex.lock().unwrap();
                        vec.push(vec![y0, y1]);
                        thread::sleep(Duration::from_micros(1));
                    }
                    _ => {
                        error!("Failed to read line!")
                    }
                }
                ix += 1;
            }
        }
        Err(e) => {
            error!("Failed to open CSV file with error {}", e);
        }

    }

     */
}