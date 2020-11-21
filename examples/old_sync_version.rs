extern crate rustic_typster;

use rustic_typster::*;

fn main() {
    // let filename = "rime_of_the_ancient_mariner.txt".into();
    // let lines = get_lines(filename).unwrap_or(Vec::new());

    let lines = fetch::Lines::new(100);

    //start(lines.iter().map(|s| s.into())).expect("BIG WOOPS");
    start(lines).expect("BIG WOOPS");

    // loop {
    //     match lines.next() {
    //         Some(x) => println!("{: <4}: {}",x.len(),x),
    //         None => break,
    //     }
    // }
    //
    // println!("{:?}",lines);
}
