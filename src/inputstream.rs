
/*
trait DataInputStream {
    fn audit(&mut self);
    fn index(&mut self) -> Option<(f64, f64)>;
}
*/
struct SineInput {
    pub deque: Arc<Mutex<VecDeque<(f64, f64)>>>,
    index: usize,
}

struct PhotometeryInput {
    pub deque: Arc<Mutex<VecDeque<(f64, f64)>>>,
    index: usize,
    port: Box<dyn SerialPort>,
}

impl SineInput {
    fn new() -> Self {
        SineInput {
            deque: Arc::new(Mutex::new(VecDeque::new())),
            index: 0,
        }
    }
}
/*
impl PhotometeryInput {
    fn new() -> Self {
        PhotometeryInput {
            deque: Arc::new(Mutex::new(VecDeque::new())),
            index: 0,
            port: PhotometeryInput::init_port()
        }
    }

    pub fn init_port() -> Box<dyn SerialPort> {
        let port = "COM3";
        let baud_rate = 9600;
        let ports = serialport::available_ports().expect("No ports found!");
        let port = serialport::new(port, baud_rate)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");

        port
    }

    pub fn audit(&mut self) {
        self.index += 1;
        let reader = std::io::BufReader::new(&mut self.port);

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    // Here you can parse the line as per your serialization format.
                    // Assuming it's a string of integers separated by spaces:
                    let numbers: Vec<i32> = line
                        .split_whitespace()
                        .filter_map(|num| num.parse::<i32>().ok())
                        .collect();
                    println!("{:?}", numbers);
                    self.deque.lock().unwrap().push_back((self.index as f64 / 10.0, numbers[0] as f64));
                    thread::sleep(Duration::from_millis(1));
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                    continue;
                },
            }
        }


        //println!("At audit method {}", ix);
        //thread::sleep(Duration::from_millis(1));
    }

    fn index(&mut self) -> Option<(f64, f64)> {
        println!("At index method");
        self.deque.lock().unwrap().pop_front()
    }
}

impl SineInput {
    pub fn audit(&mut self) {
            self.index += 1;
            let x = (self.index as f64 / 10.0);
            let y = (self.index as f64 / 10.0).sin();

            self.deque.lock().unwrap().push_back((x,y));
            //println!("At audit method {}", ix);
            //thread::sleep(Duration::from_millis(1));
        }


    fn index(&mut self) -> Option<(f64, f64)> {
        //println!("At index method");
        self.deque.lock().unwrap().pop_front()
    }
}
*/