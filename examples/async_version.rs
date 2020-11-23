extern crate rustic_typster;

use rustic_typster::*;

fn main() {
    //fetch_async::start_rustic_typster();
    type_async::main_type_async().expect("big oops");
}
