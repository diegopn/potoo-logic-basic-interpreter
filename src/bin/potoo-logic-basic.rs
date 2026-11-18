#![forbid(unsafe_code)]

use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use potoo_logic_basic_interpreter::{Host, HostValue, Operation, Request, run_with_host};

const USAGE: &str = "Usage: potoo-logic-basic <program.lbc>";

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let Some(path) = arguments.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    if path == "--help" || path == "-h" {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let path = Path::new(&path);
    match execute(path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            ExitCode::FAILURE
        }
    }
}

fn execute(path: &Path) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(path)?;
    let program_directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut host = ConsoleHost::new(program_directory);
    let window = run_with_host(&source, &mut host)?;
    if let Some(window) = window {
        write!(io::stdout().lock(), "{window}")?;
    }
    Ok(())
}

struct ConsoleHost {
    random_state: u64,
    program_directory: PathBuf,
}

impl ConsoleHost {
    fn new(program_directory: PathBuf) -> Self {
        let time_seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos() as u64);
        Self {
            random_state: time_seed ^ u64::from(std::process::id()),
            program_directory,
        }
    }

    fn next_random(&mut self) -> u64 {
        let mut state = self.random_state;
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        self.random_state = state;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn random_below(&mut self, upper: u64) -> u64 {
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let product = u128::from(self.next_random()) * u128::from(upper);
            if product as u64 >= threshold {
                return (product >> 64) as u64;
            }
        }
    }
}

impl Host for ConsoleHost {
    fn request(&mut self, request: Request<'_>) -> io::Result<Option<HostValue>> {
        match request.operation {
            Operation::InputText => {
                let prompt = request
                    .arguments
                    .first()
                    .map(|argument| argument.value.text())
                    .unwrap_or_default();
                let mut stderr = io::stderr().lock();
                write!(stderr, "{window}{prompt} ", window = request.window)?;
                stderr.flush()?;
                let mut text = String::new();
                if io::stdin().read_line(&mut text)? == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "no input available",
                    ));
                }
                if text.ends_with('\n') {
                    text.pop();
                    if text.ends_with('\r') {
                        text.pop();
                    }
                }
                Ok(Some(HostValue::Text(text)))
            }
            Operation::Message => {
                if let Some(argument) = request.arguments.first() {
                    eprintln!("{}", argument.value.text());
                }
                Ok(None)
            }
            Operation::WaitKey => {
                let mut stderr = io::stderr().lock();
                write!(stderr, "{}", request.window)?;
                stderr.flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if input.ends_with('\n') {
                    input.pop();
                    if input.ends_with('\r') {
                        input.pop();
                    }
                }
                let key = input.chars().next().into_iter().collect();
                Ok(Some(HostValue::Text(key)))
            }
            Operation::WaitSeconds => {
                if let Some(seconds) = request
                    .arguments
                    .iter()
                    .filter_map(|argument| match &argument.value {
                        HostValue::Number(value) => Some(*value),
                        _ => None,
                    })
                    .next()
                {
                    if seconds >= u64::MAX as f64 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "wait duration is out of range",
                        ));
                    }
                    if seconds > 0.0 {
                        std::thread::sleep(Duration::from_secs_f64(seconds));
                    }
                }
                Ok(None)
            }
            Operation::Random => {
                let Some(HostValue::Number(maximum)) =
                    request.arguments.first().map(|argument| &argument.value)
                else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Aleatorio requires a numeric maximum",
                    ));
                };
                if !maximum.is_finite()
                    || *maximum < 0.0
                    || maximum.fract() != 0.0
                    || *maximum > 9_007_199_254_740_991.0
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Aleatorio maximum must be an integer from 0 to 2^53-1",
                    ));
                }
                let maximum = *maximum as u64;
                let result = self.random_below(maximum + 1);
                Ok(Some(HostValue::Number(result as f64)))
            }
            operation => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("{operation:?} requires a host implementation"),
            )),
        }
    }

    fn load_program(&mut self, path: &str) -> io::Result<String> {
        let mut path = PathBuf::from(path);
        if !path.is_absolute() {
            path = self.program_directory.join(path);
        }
        if path.extension().is_none() {
            path.set_extension("lbc");
        }
        std::fs::read_to_string(path)
    }
}
