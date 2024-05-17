use std::i32::MAX;

fn main() {
    let input = [23, 82, 16, 45, 21, 94, 12, 34];

    // TODO
    let mut max = 0;
    let mut min = MAX;
    for number in input {
        println!("{}", number);
        if number > max {
            max = number
        }
        if number < min {
            min = number
        }
    }

    println!("{} is largest and {} is smallest", max, min);
}
