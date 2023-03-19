#[macro_use(defer)] extern crate scopeguard;

use anyhow::anyhow;

pub mod line_gen;
pub mod game;

fn main() {
    let args : Vec<String> = std::env::args().collect();
    match if args.len() > 1 {
        if args[1] == "dump" {
           line_gen::dump()
        } else {
            Err(anyhow!("argument not recognized"))
        }
    } else {
        game::run()
    } {
        Ok(_) => {},
        Err(e) => {
            eprintln!("\nProgram ended with error: {:#?}",e);
        },
    }
}
