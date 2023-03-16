extern crate rustic_typster;

fn main() {
    let args : Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        if args[1] == "test" {
            crate::rustic_typster::test_rustic_typster();
        } else {
            eprintln!("argument not recognized");
        }
    } else {
        crate::rustic_typster::run_rustic_typster();
    }
}
