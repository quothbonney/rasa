const STDDEV: f64 = 50.0;

use std::collections::VecDeque;

pub fn average_vec(vec: &Vec<Vec<f64>>) -> (f64, f64) {
    let (sum0, count0) = vec.iter()
        .filter_map(|inner| inner.get(0))  // get first element, if it exists
        .fold((0.0, 0), |(sum, count), &val| (sum + val, count + 1)); // sum values and count elements

    let (sum1, count1) = vec.iter()
        .filter_map(|inner| inner.get(1))  // get first element, if it exists
        .fold((0.0, 0), |(sum, count), &val| (sum + val, count + 1)); // sum values and count elements

    let av = (sum0 / (count0 as f64), (sum1 / (count1 as f64)));
    av
}


pub fn normalize_array(lower_: &Vec<f64>, high_: &Vec<f64>) -> (Vec<f64>, Vec<f64>) {
    let mut v0 = lower_.clone();
    let mut v1 = high_.clone();

    // Get each min and max value from the arrays in order to normalize them
    // There has got to be an easier way to do this, but this is the best that I can find
    // Honestly Rust is a great language but the builder pattern is not the move...
    let min_val0 = v0.iter().chain(v0.iter()).cloned().fold(f64::INFINITY, f64::min);
    let max_val0 = v0.iter().chain(v0.iter()).cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_val1 = v1.iter().chain(v1.iter()).cloned().fold(f64::INFINITY, f64::min);
    let max_val1 = v1.iter().chain(v1.iter()).cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_val = min_val0.min(min_val1);

    // Actually a mistake, but I generated the model by subtracting the min, so obviously I have to do the same thing
    let mut normalized_arr: (Vec<f64>, Vec<f64>) = (
        v0.iter().map(|&x| (x - min_val) / STDDEV).collect(),
        v1.iter().map(|&x| (x - min_val) / STDDEV).collect(),
    );

    let avg0: f64 = normalized_arr.0.iter().sum::<f64>() / normalized_arr.0.len() as f64;
    let avg1: f64 = normalized_arr.1.iter().sum::<f64>() / normalized_arr.1.len() as f64;

    normalized_arr.0.iter_mut().for_each(|x| *x -= avg0);
    // Offset by 3 for the RNN to distinguish the channels. Could have passed channels 2 at a time. Didn't.
    normalized_arr.1.iter_mut().for_each(|x| *x -= (avg1 + 3.0));

    normalized_arr
}

pub fn std_dev(data1: &[f64], data2: &[f64]) -> (f64, f64) {
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

pub fn std_dev_vec_deque(data: &VecDeque<f64>) -> Option<f64> {
    let n = data.len() as f64;

    // Calculate the mean
    let sum: f64 = data.iter().sum();
    let mean = sum / n;

    // Calculate the sum of squares of differences
    let sum_of_squares: f64 = data
        .iter()
        .map(|&x| (x - mean).powi(2))
        .sum();

    // Calculate the variance
    let variance = sum_of_squares / n;

    // Calculate the standard deviation
    let standard_deviation = variance.sqrt();

    // Return the result
    if n > 1.0 {
        Some(standard_deviation)
    } else {
        None
    }
}

pub fn average_vec_deque(data: &VecDeque<f64>) -> Option<f64> {
    let n = data.len() as f64;

    // Calculate the sum of all elements
    let sum: f64 = data.iter().sum();

    // Calculate the average (mean)
    let average = sum / n;

    // Return the result
    if n > 0.0 {
        Some(average)
    } else {
        None
    }
}