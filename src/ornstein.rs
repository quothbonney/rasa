use rand::Rng;
use rand_distr::{Normal, Distribution};

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