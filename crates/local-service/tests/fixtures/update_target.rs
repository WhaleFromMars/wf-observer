use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--version")) {
        println!("wf-observer-service 0.1.1");
        ExitCode::SUCCESS
    } else {
        eprintln!("the update target fixture only supports --version");
        ExitCode::FAILURE
    }
}
