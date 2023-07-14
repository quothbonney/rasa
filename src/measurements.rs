use std::collections::VecDeque;
use tracing::warn;

pub type Measurement = egui::plot::PlotPoint;

#[derive(Debug)]
pub struct MeasurementWindow {
    // Values is a vector of vecdeques. The first dimension (non-deque) corresponds to each DataInputStream class
    // The second data from the deque corresponds to the plottable data
    pub values: Vec<VecDeque<Measurement>>,
    pub look_behind: usize,
    pub channels: usize,
    pub  rectpoints: Vec<[f64; 2]>,
}

impl MeasurementWindow {
    pub fn new_with_look_behind(look_behind: usize, channels: usize) -> Self {
        Self {
            values: vec![VecDeque::new(); channels],
            look_behind,
            channels,
            rectpoints: vec![[0.0; 2]; 4],
        }
    }

    pub fn update_rect(&mut self, rectp: Vec<[f64; 2]>) {
        self.rectpoints = rectp;
    }

    pub fn add(&mut self, channel: usize, measurement: Measurement) {
        // Test the existence of the channel
        match self.values.get_mut(channel) {
            Some(ch) => {
                if let Some(last) = ch.back() {
                    if measurement.x < last.x {
                        ch.clear();
                    }
                }
                ch.push_back(measurement);

                let limit = measurement.x - (self.look_behind as f64);
                while let Some(front) = ch.front() {
                    if front.x >= limit {
                        break;
                    }
                    ch.pop_front();
                }
            }
            None => {
                warn!("Channel {} is out of bounds for plot with {} data streams", channel, self.values.len())
            }
        }
    }

    pub fn plot_values(&self, channel: usize) -> egui::plot::PlotPoints {
        egui::plot::PlotPoints::Owned(Vec::from_iter(self.values[channel].iter().copied()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_measurements() {
        let w = MeasurementWindow::new_with_look_behind(123);
        assert_eq!(w.values.len(), 0);
        assert_eq!(w.look_behind, 123);
    }

    #[test]
    fn appends_one_value() {
        let mut w = MeasurementWindow::new_with_look_behind(100);

        w.add(0, Measurement::new(10.0, 20.0));
        assert_eq!(
            w.values.into_iter().eq(vec![Measurement::new(10.0, 20.0)]),
            true
        );
    }

    #[test]
    fn clears_on_out_of_order() {
        let mut w = MeasurementWindow::new_with_look_behind(100);

        w.add(0, Measurement::new(10.0, 20.0));
        w.add(0, Measurement::new(20.0, 30.0));
        w.add(0, Measurement::new(19.0, 100.0));
        assert_eq!(
            w.values.into_iter().eq(vec![Measurement::new(19.0, 100.0)]),
            true
        );
    }

    #[test]
    fn appends_several_values() {
        let mut w = MeasurementWindow::new_with_look_behind(100);

        for x in 1..=20 {
            w.add(0, Measurement::new((x as f64) * 10.0, x as f64));
        }

        assert_eq!(
            w.values.into_iter().eq(vec![
                Measurement::new(100.0, 10.0),
                Measurement::new(110.0, 11.0),
                Measurement::new(120.0, 12.0),
                Measurement::new(130.0, 13.0),
                Measurement::new(140.0, 14.0),
                Measurement::new(150.0, 15.0),
                Measurement::new(160.0, 16.0),
                Measurement::new(170.0, 17.0),
                Measurement::new(180.0, 18.0),
                Measurement::new(190.0, 19.0),
                Measurement::new(200.0, 20.0),
            ]),
            true
        );
    }
}
