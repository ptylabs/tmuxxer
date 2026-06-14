use std::env;
use std::process;

fn main() {
    if let Err(e) = tmuxxer::run_cli(env::args().skip(1)) {
        if tmuxxer::should_report_error(&e) {
            eprintln!("tmuxxer: {e}");
        }
        process::exit(1);
    }
}
