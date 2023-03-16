#[macro_use(defer)] extern crate scopeguard;

pub mod fetch_async;
pub mod type_async;

#[derive(Debug)]
pub struct SrcString {
    pub string: String,
    pub source: String,
}

pub fn run_rustic_typster() {
    match type_async::main_rustic_typster() {
        Ok(_) => (),
        Err(e) => {
            eprintln!("\nProgram ended with error: {:#?}",e);
        }
    }
}
