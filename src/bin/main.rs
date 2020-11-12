extern crate rustic_typster;

use rustic_typster::*;
use crate::fetch::*;

fn main() {

    // let filename = "rime_of_the_ancient_mariner.txt".into();
    //
    // let lines = get_lines(filename).unwrap_or(Vec::new());
    //
    // start(lines).expect("BIG WOOPS");

    //fetch_docs_rs();

    let mut lines = Lines::new(10);

    loop {
        match lines.next() {
            Some(x) => println!("{: <4}: {}",x.len(),x),
            None => break,
        }
    }

    println!("{:?}",lines);

}
