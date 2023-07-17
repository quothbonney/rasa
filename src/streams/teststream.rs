use std::time::{Instant, Duration};
use std::thread;
use std::sync::mpsc::Sender;

pub fn start_test_stream(tx: Sender<[(f64, f64); 4]>) {
    let mut ix = 1i32;
    let start = Instant::now();
    loop {
        let y: f64 = match ix % 100 {
            0 => 2.0,
            _ => (ix as f64).sin()
        };
        let elapsed: f64 = (start.elapsed().as_millis() as f64) / 1000.0;
        let num = [
            (elapsed, y),
            (elapsed, y * y),
            (elapsed, 0.0),
            (elapsed, 0.0)
        ];
        //println!("{:?}", num);
        tx.send(num).unwrap();

        thread::sleep(Duration::from_micros(10));
        ix += 1;
    }
}
