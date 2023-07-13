use std::sync::*;
use std::collections::VecDeque;

pub struct BoundedSender {
    sender: mpsc::Sender<f32>,
    pub deque: Arc<Mutex<VecDeque<f32>>>,
    capacity: usize,
}

impl BoundedSender {
    pub fn send(&self, val: f32) {
        let mut deque = self.deque.lock().unwrap();

        if deque.len() == self.capacity {
            deque.pop_front();  // remove the oldest value
        }

        deque.push_back(val);  // add the new value
        self.sender.send(val).unwrap();
    }
}

pub struct BoundedReceiver {
    receiver: mpsc::Receiver<f32>,
    pub(crate) deque: Arc<Mutex<VecDeque<f32>>>,
}

impl BoundedReceiver {
    pub fn recv(&self) -> Option<f32> {
        match self.receiver.recv() {
            Ok(val) => {
                let mut deque = self.deque.lock().unwrap();
                deque.pop_front();  // remove the received value from the buffer
                Some(val)
            },
            Err(_) => None,
        }
    }
}

pub fn deque_channel(capacity: usize) -> (BoundedSender, BoundedReceiver) {
    let (tx, rx) = mpsc::channel();
    let deque = Arc::new(Mutex::new(VecDeque::with_capacity(capacity)));

    (
        BoundedSender {
            sender: tx,
            deque: Arc::clone(&deque),
            capacity
        },
        BoundedReceiver {
            receiver: rx,
            deque,
        },
    )
}
