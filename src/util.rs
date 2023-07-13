
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