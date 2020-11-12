extern crate rustic_typster;

use rustic_typster::*;

fn main() {

    let filename = "rime_of_the_ancient_mariner.txt".into();

    let lines = get_lines(filename).unwrap_or(Vec::new());

    start(lines).expect("BIG WOOPS");

}
