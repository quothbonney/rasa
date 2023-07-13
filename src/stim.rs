
pub fn qualifies_for_stimulation(v0: &[f64], v1: &[f64], averages: (f64, f64), stddevs: (f64, f64), percentage: f64) -> bool {
    let sigma_level = 1.0;
    let target0 = averages.0 + (sigma_level * stddevs.0);
    let target1 = averages.1 + (sigma_level * stddevs.1);

    //println!("{}, {}", target0, target1);
    let mut count = (0i32, 0i32);
    let mut total = (0i32, 0i32);

    for i in v0 {
        if i < &target0 { count.0 += 1;};
        total.0 += 1;
    }

    for j in v1 {
        if j < &target1 { count.1 += 1;};
        total.1 += 1;
    }

    //println!("Counts: {}, {}", count.0, count.1);

    let percentage_to_target = ((count.0 as f64 / total.0 as f64), (count.1 as f64 / total.1 as f64));
    //println!("{:?}", percentage_to_target);
    percentage_to_target.0 > percentage && percentage_to_target.0 > percentage

}