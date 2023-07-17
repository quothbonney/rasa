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

use crate::threadedchannel::{BoundedSender, deque_channel};
use crate::util::*;

pub struct OrnsteinUhlenbeck {
    theta: f64,
    mu: f64,
    sigma: f64,
    x: f64,
}

impl OrnsteinUhlenbeck {
    pub fn new(theta: f64, mu: f64, sigma: f64, x0: f64) -> Self {
        OrnsteinUhlenbeck {
            theta,
            mu,
            sigma,
            x: x0,
        }
    }

    pub fn step(&mut self, dt: f64) -> f64 {
        let mut rng = rand::thread_rng();
        let normal = Normal::new(0.0, (2.0 * self.theta * self.sigma * dt).sqrt()).unwrap();
        let dw = normal.sample(&mut rng);
        let dx = self.theta * (self.mu - self.x) * dt + self.sigma * dw;
        self.x += dx;
        self.x
    }
}


pub fn start_ornstein_stream(tx: Sender<[(f64, f64); 4]>, tx_deque0: &BoundedSender, tx_deque1: &BoundedSender, tx_time: &BoundedSender, mut writer: Writer<File>,is_ttl: Arc<Mutex<bool>>) {
    let mut ttl_guard = is_ttl.lock().unwrap();
    *ttl_guard = true;

    info!("Beginning Ornstein stream on active thread");
    let mut zapper_timer = Instant::now();
    let start = Instant::now();
    let mut process = OrnsteinUhlenbeck::new(0.5, 0.5, 0.1, 0.0);
    let dt = 0.01;
    let mut sec_start = Instant::now();
    let mut old_average = (0f64, 0f64);
    let mut old_std = (0f64, 0f64);
    let y0: f64;
    let y1: f64;


    //let reader = std::io::BufReader::new(port);
    let mut ix = 1i32;
    loop {
        let y0: f64 = process.step(dt) * 50.0;
        let y1 = y0 - 10.0;
        let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
        let num = [
            (elapsed, y0),
            (elapsed, y1),
            (elapsed - 1.0, old_average.0),
            (elapsed -1.0, old_average.1)
        ];
        tx.send(num).unwrap();
        tx_deque0.send(y0 as f32);
        tx_deque1.send(y1 as f32);
        tx_time.send(elapsed as f32);

        if sec_start.elapsed() > Duration::from_secs(1) {
            sec_start = Instant::now();
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
}
