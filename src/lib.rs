#![forbid(unsafe_code)]

//! Execute programs in the documented subset of Logic Basic 7.0.

mod host;
mod syntax;
mod window;

pub use host::{
    Argument as HostArgument, Host, Memory as HostMemory, Operation, Record, Records, Request,
    Snapshot, Value as HostValue,
};
pub use window::Window;

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io;

use syntax::{Arithmetic, Command, Comparison, Condition, Expression, Operand, Value};

struct Database {
    directory: String,
    fields: Vec<(String, usize)>,
    file: Option<String>,
    records: Option<Records>,
    order: Option<Vec<String>>,
    current: Record,
}

type Databases = HashMap<String, Database>;

/// A source or execution error associated with a physical line of the program.
#[derive(Debug)]
pub struct Error {
    line: usize,
    message: String,
    stopped: bool,
}

impl Error {
    pub(crate) fn new(line: usize, message: String) -> Self {
        Self {
            line,
            message,
            stopped: false,
        }
    }

    fn from_io(line: usize, context: &str, cause: io::Error) -> Self {
        let stopped = cause.kind() == io::ErrorKind::Interrupted;
        Self {
            line,
            message: format!("{context}: {cause}"),
            stopped,
        }
    }

    /// Returns the one-based source line, including for commands after a semicolon.
    pub fn line(&self) -> usize {
        self.line
    }

    /// Returns whether the host interrupted this execution.
    pub fn was_stopped(&self) -> bool {
        self.stopped
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for Error {}

/// Executes a program, using the supplied callback for `EntraTexto`.
///
/// Returns the text window at exhaustion or `FimPrograma`, and `None` when
/// `FimJanela` closes it. The callback receives the prompt and current window.
pub fn run(
    source: &str,
    read_text: impl FnMut(&str, &Window) -> io::Result<String>,
) -> Result<Option<Window>, Error> {
    let mut host = CallbackHost(read_text);
    run_with_host(source, &mut host)
}

/// Executes a program with host-provided input, graphics, files, and data services.
pub fn run_with_host(source: &str, host: &mut dyn Host) -> Result<Option<Window>, Error> {
    let program = syntax::parse(source)?;
    let mut window = Window::default();
    let mut memories = HashMap::new();
    memories.insert("RETORNO".to_owned(), Value::Text(String::new()));
    let mut databases = HashMap::new();
    let last_line = program.last().map_or(1, |statement| statement.line);
    let result = execute_program(&program, host, &mut window, &mut memories, &mut databases);
    let cleanup = close_open_databases(&mut databases, host);
    match result {
        Err(error) => Err(error),
        Ok(window_open) => {
            cleanup.map_err(|message| Error {
                line: last_line,
                message,
                stopped: false,
            })?;
            Ok(window_open.then_some(window))
        }
    }
}

fn execute_program(
    program: &[syntax::Statement],
    host: &mut dyn Host,
    window: &mut Window,
    memories: &mut Memories,
    databases: &mut Databases,
) -> Result<bool, Error> {
    let mut current = 0;
    let mut returns = Vec::new();
    while let Some(statement) = program.get(current) {
        let error = |message| Error {
            line: statement.line,
            message,
            stopped: false,
        };
        host.checkpoint(statement.line)
            .map_err(|cause| Error::from_io(statement.line, "execution checkpoint", cause))?;
        current += 1;
        match &statement.command {
            Command::Declare(names) => {
                let mut array_elements = total_array_elements(memories);
                for (name, size) in names {
                    if let Some(Value::Array(existing)) = memories.get(name) {
                        array_elements = array_elements.saturating_sub(existing.len());
                    }
                    if let Some(size) = size {
                        array_elements = array_elements
                            .checked_add(*size)
                            .filter(|total| *total <= MAX_ARRAY_ELEMENTS)
                            .ok_or_else(|| {
                                error(format!(
                                    "total array storage cannot exceed {MAX_ARRAY_ELEMENTS} elements"
                                ))
                            })?;
                    }
                    let value = match size {
                        Some(size) => Value::Array(vec![Some(Value::Text(String::new())); *size]),
                        None => Value::Text(String::new()),
                    };
                    memories.insert(name.clone(), value);
                }
            }
            Command::Assign(name, expression) => {
                let value =
                    evaluate(expression, &memories, &databases, host, &window).map_err(error)?;
                set_memory(memories, name, value).map_err(error)?;
            }
            Command::ArrayAssign(name, index, expression) => {
                let index = evaluate(index, &memories, &databases, host, &window)
                    .and_then(|value| value.index())
                    .map_err(error)?;
                let value =
                    evaluate(expression, &memories, &databases, host, &window).map_err(error)?;
                let Some(Value::Array(items)) = memories.get_mut(name) else {
                    return Err(error("indexed assignment requires an array memory".into()));
                };
                let Some(slot) = items.get_mut(index) else {
                    return Err(error(format!("array index {index} is out of range")));
                };
                *slot = Some(value);
            }
            Command::FindArray(destination, name, target) => {
                let target = evaluate_operand(target, &memories, &databases, host, &window)
                    .map_err(error)?;
                let result = match memories.get(name) {
                    Some(Value::Array(items)) => items
                        .iter()
                        .position(|value| value.as_ref() == Some(&target))
                        .map_or(-1.0, |index| index as f64),
                    Some(_) => return Err(error(format!("{name} is not an array memory"))),
                    None => return Err(error(format!("unknown matrix {name}"))),
                };
                set_memory(memories, destination, Value::Number(result)).map_err(error)?;
            }
            Command::Update(name, operator, expression) => {
                let right =
                    evaluate(expression, &memories, &databases, host, &window).map_err(error)?;
                let left = memories
                    .get(name)
                    .ok_or_else(|| error(format!("unknown memory {name}")))?;
                let value = calculate(left, *operator, &right).map_err(error)?;
                set_memory(memories, name, value).map_err(error)?;
            }
            Command::ReadText(name, prompt) => {
                let prompt = evaluate_operand(prompt, &memories, &databases, host, &window)
                    .map_err(error)?
                    .text();
                let arguments = [HostArgument {
                    name: None,
                    value: HostValue::Text(prompt),
                }];
                let response = host
                    .request(Request {
                        operation: Operation::InputText,
                        arguments: &arguments,
                        rows: &[],
                        window: &window,
                    })
                    .map_err(|cause| Error::from_io(statement.line, "EntraTexto", cause))?;
                let text = response
                    .ok_or_else(|| error("EntraTexto did not return text".into()))?
                    .text();
                set_memory(memories, name, Value::Text(text)).map_err(error)?;
            }
            Command::Position(row, column) => {
                let row =
                    evaluate_operand(row, &memories, &databases, host, &window).map_err(error)?;
                let column = evaluate_operand(column, &memories, &databases, host, &window)
                    .map_err(error)?;
                window
                    .position(
                        row.number().map_err(error)?,
                        column.number().map_err(error)?,
                    )
                    .map_err(error)?;
                host.window_changed(&window)
                    .map_err(|cause| Error::from_io(statement.line, "Posicione", cause))?;
            }
            Command::Write(items) => {
                let text: String = items
                    .iter()
                    .map(|item| {
                        evaluate_operand(item, &memories, &databases, host, &window)
                            .map(|value| value.text())
                    })
                    .collect::<Result<_, _>>()
                    .map_err(error)?;
                window.write(&text);
                host.window_changed(&window)
                    .map_err(|cause| Error::from_io(statement.line, "Escreva", cause))?;
            }
            Command::Clear => {
                window.clear();
                host.window_changed(&window)
                    .map_err(|cause| Error::from_io(statement.line, "LimpaJanela", cause))?;
            }
            Command::Message(value) => {
                let arguments = [HostArgument {
                    name: None,
                    value: to_host_value(
                        evaluate_operand(value, &memories, &databases, host, &window)
                            .map_err(error)?,
                    ),
                }];
                host.request(Request {
                    operation: Operation::Message,
                    arguments: &arguments,
                    rows: &[],
                    window: &window,
                })
                .map_err(|cause| Error::from_io(statement.line, "Mensagem", cause))?;
            }
            Command::Wait(values) => {
                let arguments: Vec<_> = values
                    .iter()
                    .map(|operand| {
                        evaluate_operand(operand, &memories, &databases, host, &window).map(
                            |value| HostArgument {
                                name: operand.name.clone(),
                                value: to_host_value(value),
                            },
                        )
                    })
                    .collect::<Result<_, _>>()
                    .map_err(error)?;
                let kind = arguments
                    .first()
                    .map(|argument| argument.value.text().to_ascii_uppercase());
                let operation = match arguments.first().map(|argument| &argument.value) {
                    Some(HostValue::Number(_)) => Operation::WaitSeconds,
                    _ => match kind.as_deref() {
                        Some("CLIQUEOBJETO") => Operation::WaitObject,
                        Some("CLIQUEBOTAO") => Operation::WaitButton,
                        _ => Operation::WaitKey,
                    },
                };
                if operation == Operation::WaitSeconds {
                    let Some(HostValue::Number(seconds)) =
                        arguments.first().map(|argument| &argument.value)
                    else {
                        return Err(error("Aguarde seconds requires a number".into()));
                    };
                    if !seconds.is_finite() || *seconds < 0.0 {
                        return Err(error(
                            "Aguarde seconds must be finite and nonnegative".into(),
                        ));
                    }
                }
                let result = host
                    .request(Request {
                        operation,
                        arguments: if operation == Operation::WaitSeconds {
                            &arguments
                        } else {
                            &[]
                        },
                        rows: &[],
                        window: &window,
                    })
                    .map_err(|cause| Error::from_io(statement.line, "Aguarde", cause))?;
                if let Some(value) = result {
                    match values.first().map(|argument| &argument.value) {
                        Some(syntax::Atom::Memory(name)) => {
                            set_memory(memories, name, from_host_value(value)).map_err(error)?;
                        }
                        Some(syntax::Atom::Literal(Value::Text(kind)))
                            if kind.eq_ignore_ascii_case("CLIQUEOBJETO")
                                || kind.eq_ignore_ascii_case("CLIQUEBOTAO") =>
                        {
                            set_memory(memories, "RETORNO", from_host_value(value))
                                .map_err(error)?;
                        }
                        _ => {}
                    }
                }
            }
            Command::Host(operation, values) => {
                let arguments: Vec<_> = values
                    .iter()
                    .map(|operand| {
                        evaluate_operand(operand, &memories, &databases, host, &window).map(
                            |value| HostArgument {
                                name: operand.name.clone(),
                                value: to_host_value(value),
                            },
                        )
                    })
                    .collect::<Result<_, _>>()
                    .map_err(error)?;
                let mut configured_window = None;
                if *operation == Operation::ConfigureWindow {
                    if let Some((rows, columns)) = window_resolution(&arguments).map_err(error)? {
                        let mut resized = window.clone();
                        resized.resize(rows, columns).map_err(error)?;
                        configured_window = Some(resized);
                    }
                }
                let request_window = configured_window.as_ref().unwrap_or(window);
                host.request(Request {
                    operation: *operation,
                    arguments: &arguments,
                    rows: &[],
                    window: request_window,
                })
                .map_err(|cause| {
                    Error::from_io(statement.line, &format!("{operation:?}"), cause)
                })?;
                if let Some(resized) = configured_window {
                    *window = resized;
                    host.window_changed(window)
                        .map_err(|cause| Error::from_io(statement.line, "Janela", cause))?;
                }
            }
            Command::HostAssign(destination, operation, values) => {
                let arguments: Vec<_> = values
                    .iter()
                    .map(|operand| {
                        evaluate_operand(operand, &memories, &databases, host, &window).map(
                            |value| HostArgument {
                                name: operand.name.clone(),
                                value: to_host_value(value),
                            },
                        )
                    })
                    .collect::<Result<_, _>>()
                    .map_err(error)?;
                let value = host
                    .request(Request {
                        operation: *operation,
                        arguments: &arguments,
                        rows: &[],
                        window: &window,
                    })
                    .map_err(|cause| {
                        Error::from_io(statement.line, &format!("{operation:?}"), cause)
                    })?
                    .ok_or_else(|| error(format!("{operation:?} did not return a value")))?;
                set_memory(memories, destination, from_host_value(value)).map_err(error)?;
            }
            Command::HostBlock(operation, header, rows) => {
                let arguments: Vec<_> = header
                    .iter()
                    .map(|operand| {
                        evaluate_operand(operand, &memories, &databases, host, &window).map(
                            |value| HostArgument {
                                name: operand.name.clone(),
                                value: to_host_value(value),
                            },
                        )
                    })
                    .collect::<Result<_, _>>()
                    .map_err(error)?;
                let rows: Vec<Vec<_>> =
                    rows.iter()
                        .map(|row| {
                            row.iter()
                                .map(|operand| {
                                    evaluate_operand(operand, &memories, &databases, host, &window)
                                        .map(|value| HostArgument {
                                            name: operand.name.clone(),
                                            value: to_host_value(value),
                                        })
                                })
                                .collect::<Result<_, _>>()
                        })
                        .collect::<Result<_, _>>()
                        .map_err(error)?;
                if *operation == Operation::DatabaseDefine {
                    if arguments.len() != 2 {
                        return Err(error("BD requires a name and data directory".into()));
                    }
                    let name = arguments
                        .first()
                        .ok_or_else(|| error("BD requires a name".into()))?
                        .value
                        .text()
                        .to_ascii_uppercase();
                    if name.is_empty() {
                        return Err(error("BD requires a nonempty name".into()));
                    }
                    let directory = arguments
                        .get(1)
                        .ok_or_else(|| error("BD requires a data directory".into()))?
                        .value
                        .text();
                    let fields = rows
                        .iter()
                        .map(|row| {
                            if row.len() != 2 {
                                return Err(error("BD field rows need a name and width".into()));
                            }
                            let name = row
                                .first()
                                .ok_or_else(|| error("BD field requires a name".into()))?
                                .value
                                .text();
                            let size = row
                                .get(1)
                                .ok_or_else(|| error("BD field requires a width".into()))?
                                .value
                                .number()
                                .map_err(error)?;
                            if size < 1.0 || size.fract() != 0.0 || size > usize::MAX as f64 {
                                return Err(error(
                                    "BD field width must be a positive integer".into(),
                                ));
                            }
                            Ok((name, size as usize))
                        })
                        .collect::<Result<Vec<_>, Error>>()?;
                    let mut field_names = HashSet::new();
                    if fields.iter().any(|(field, _)| {
                        field.trim().is_empty() || !field_names.insert(field.to_ascii_uppercase())
                    }) {
                        return Err(error("BD field names must be nonempty and unique".into()));
                    }
                    if databases.contains_key(&name) {
                        return Err(error(format!("duplicate database {name}")));
                    }
                    let current = blank_record(&fields);
                    databases.insert(
                        name,
                        Database {
                            directory,
                            fields,
                            file: None,
                            records: None,
                            order: None,
                            current,
                        },
                    );
                    continue;
                }
                let response = host
                    .request(Request {
                        operation: *operation,
                        arguments: &arguments,
                        rows: &rows,
                        window: &window,
                    })
                    .map_err(|cause| error(format!("{operation:?}: {cause}")))?;
                if let Some(value) = response {
                    set_memory(memories, "RETORNO", from_host_value(value)).map_err(error)?;
                }
            }
            Command::CreateResources(operation, resources) => {
                for (memory, path) in resources {
                    let path = evaluate_operand(path, &memories, &databases, host, &window)
                        .map_err(error)?
                        .text();
                    let arguments = [
                        HostArgument {
                            name: None,
                            value: HostValue::Text(memory.clone()),
                        },
                        HostArgument {
                            name: None,
                            value: HostValue::Text(path),
                        },
                    ];
                    let result = host
                        .request(Request {
                            operation: *operation,
                            arguments: &arguments,
                            rows: &[],
                            window: &window,
                        })
                        .map_err(|cause| {
                            Error::from_io(statement.line, &format!("{operation:?}"), cause)
                        })?;
                    let value = match (operation, result) {
                        (Operation::CreateObject, Some(HostValue::Object(key))) => {
                            Value::Object(key)
                        }
                        (Operation::CreateSound, Some(HostValue::Sound(key))) => Value::Sound(key),
                        (Operation::CreateObject, Some(value)) => {
                            return Err(error(format!(
                                "object creation returned unexpected value {value:?}"
                            )));
                        }
                        (Operation::CreateSound, Some(value)) => {
                            return Err(error(format!(
                                "sound creation returned unexpected value {value:?}"
                            )));
                        }
                        (Operation::CreateObject, None) => Value::Object(memory.clone()),
                        (Operation::CreateSound, None) => Value::Sound(memory.clone()),
                        _ => return Err(error("invalid resource creation operation".into())),
                    };
                    set_memory(memories, memory, value).map_err(error)?;
                }
            }
            Command::Database(operation, operands) => {
                let arguments = operands
                    .iter()
                    .map(|operand| evaluate_operand(operand, &memories, &databases, host, &window))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(error)?;
                execute_database(*operation, &arguments, databases, host, &window)
                    .map_err(error)?;
            }
            Command::DatabaseFieldAssign(database, field, expression) => {
                let value =
                    evaluate(expression, &memories, &databases, host, &window).map_err(error)?;
                let table = databases
                    .get_mut(database)
                    .ok_or_else(|| error(format!("unknown database {database}")))?;
                let canonical = table
                    .fields
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(field))
                    .map(|(name, _)| name.clone())
                    .ok_or_else(|| error(format!("unknown field {database}.{field}")))?;
                table.current.insert(canonical, to_host_value(value));
            }
            Command::SaveMemories(path) => {
                let path = evaluate_operand(path, &memories, &databases, host, &window)
                    .map_err(error)?
                    .text();
                let snapshot = memories
                    .iter()
                    .filter(|(name, _)| name.as_str() != "RETORNO")
                    .map(|(name, value)| (name.clone(), to_host_memory(value.clone())))
                    .collect();
                host.save_memories(&path, &snapshot)
                    .map_err(|cause| Error::from_io(statement.line, "GravaMemoria", cause))?;
            }
            Command::LoadMemories(path) => {
                let path = evaluate_operand(path, &memories, &databases, host, &window)
                    .map_err(error)?
                    .text();
                if let Some(snapshot) = host
                    .load_memories(&path)
                    .map_err(|cause| Error::from_io(statement.line, "CarregaMemoria", cause))?
                {
                    for (name, value) in snapshot {
                        let name = name.to_ascii_uppercase();
                        if name == "RETORNO" {
                            continue;
                        }
                        let mut array_elements = total_array_elements(memories);
                        if let Some(Value::Array(existing)) = memories.get(&name) {
                            array_elements = array_elements.saturating_sub(existing.len());
                        }
                        if let HostMemory::Array(values) = &value {
                            array_elements = array_elements
                                .checked_add(values.len())
                                .filter(|total| *total <= MAX_ARRAY_ELEMENTS)
                                .ok_or_else(|| {
                                    error(format!(
                                        "loaded memories exceed the {MAX_ARRAY_ELEMENTS}-element array limit"
                                    ))
                                })?;
                        }
                        memories.insert(name, from_host_memory(value));
                    }
                }
            }
            Command::LoadProgram(path) => {
                let path = evaluate_operand(path, memories, databases, host, window)
                    .map_err(error)?
                    .text();
                let source = host
                    .load_program(&path)
                    .map_err(|cause| Error::from_io(statement.line, "CarregaPrograma", cause))?;
                let loaded = syntax::parse(&source)
                    .map_err(|cause| error(format!("CarregaPrograma: {cause}")))?;
                if !execute_program(&loaded, host, window, memories, databases)? {
                    return Ok(false);
                }
            }
            Command::RemoveMemory(name) => {
                if name == "RETORNO" || memories.remove(name).is_none() {
                    return Err(error(format!("cannot remove memory {name}")));
                }
            }
            Command::ReorganizeMemories => {
                // Named memories have no positional storage table to compact.
            }
            Command::If(condition, otherwise) => {
                if !check(condition, &memories, &databases, host, &window).map_err(error)? {
                    current = *otherwise;
                }
            }
            Command::Jump(destination) => current = *destination,
            Command::Call(destination) => {
                returns.push(current);
                current = *destination;
            }
            Command::RoutineStart => {}
            Command::Return => {
                current = returns
                    .pop()
                    .ok_or_else(|| error("Retorne without an active routine".into()))?;
            }
            Command::EndProgram => break,
            Command::EndWindow => return Ok(false),
        }
    }
    Ok(true)
}

fn blank_record(fields: &[(String, usize)]) -> Record {
    fields
        .iter()
        .map(|(name, _)| (name.clone(), HostValue::Text(String::new())))
        .collect()
}

fn set_memory(memories: &mut Memories, name: &str, value: Value) -> Result<(), String> {
    let memory = memories
        .get_mut(name)
        .ok_or_else(|| format!("unknown memory {name}"))?;
    *memory = value;
    Ok(())
}

fn total_array_elements(memories: &Memories) -> usize {
    memories.values().fold(0usize, |total, memory| {
        total.saturating_add(match memory {
            Value::Array(values) => values.len(),
            _ => 0,
        })
    })
}

fn window_resolution(arguments: &[HostArgument]) -> Result<Option<(usize, usize)>, String> {
    let resolution: Vec<_> = arguments
        .iter()
        .filter(|argument| {
            argument
                .name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case("RESOLUCAO"))
        })
        .collect();
    if resolution.is_empty() {
        return Ok(None);
    }
    if resolution.len() != 2 {
        return Err("Janela Resolucao requires a row and column count".into());
    }
    let dimension = |argument: &HostArgument, name: &str| -> Result<usize, String> {
        let value = argument.value.number()?;
        if value < 1.0 || value.fract() != 0.0 || value >= usize::MAX as f64 {
            return Err(format!(
                "Janela Resolucao {name} must be a positive integer"
            ));
        }
        Ok(value as usize)
    };
    Ok(Some((
        dimension(resolution[0], "rows")?,
        dimension(resolution[1], "columns")?,
    )))
}

fn normalize_record(fields: &[(String, usize)], record: &Record) -> Record {
    fields
        .iter()
        .map(|(field, _)| {
            let value = record
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(field))
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| HostValue::Text(String::new()));
            (field.clone(), value)
        })
        .collect()
}

fn build_record(fields: &[(String, usize)], current: &Record) -> Record {
    fields
        .iter()
        .map(|(field, width)| {
            let value = current
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(field))
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| HostValue::Text(String::new()));
            let text = value.text();
            let bounded = if text.chars().count() > *width {
                HostValue::Text(text.chars().take(*width).collect())
            } else {
                value
            };
            (field.clone(), bounded)
        })
        .collect()
}

fn close_open_databases(databases: &mut Databases, host: &mut dyn Host) -> Result<(), String> {
    for (name, table) in databases {
        let Some(file) = table.file.clone() else {
            continue;
        };
        let records = table
            .records
            .as_ref()
            .ok_or_else(|| format!("database {name} has a filename but is not open"))?;
        host.save_database(&table.directory, &file, records)
            .map_err(|cause| format!("database {name} save on close: {cause}"))?;
        host.close_database(&table.directory, &file)
            .map_err(|cause| format!("database {name} close: {cause}"))?;
        table.file = None;
        table.records = None;
    }
    Ok(())
}

fn execute_database(
    operation: Operation,
    arguments: &[Value],
    databases: &mut Databases,
    host: &mut dyn Host,
    window: &Window,
) -> Result<(), String> {
    let name = arguments
        .first()
        .ok_or_else(|| format!("{operation:?} requires a database name"))?
        .text()
        .to_ascii_uppercase();
    let table = databases
        .get(&name)
        .ok_or_else(|| format!("unknown database {name}"))?;
    match operation {
        Operation::DatabaseOpen => {
            if table.records.is_some() {
                return Err(format!("database {name} is already open"));
            }
            let directory = table.directory.clone();
            let fields = table.fields.clone();
            let file = arguments
                .get(1)
                .map(Value::text)
                .unwrap_or_else(|| name.clone());
            let file = if file.to_ascii_lowercase().ends_with(".lba") {
                file
            } else {
                format!("{file}.LBA")
            };
            let mut records = host
                .open_database(&directory, &file, &fields)
                .map_err(|cause| format!("AbreBD: {cause}"))?;
            for record in records.iter_mut().flatten() {
                *record = normalize_record(&fields, record);
            }
            let table = databases
                .get_mut(&name)
                .expect("database remains registered while opening");
            table.file = Some(file);
            table.records = Some(records);
            table.current = blank_record(&table.fields);
        }
        Operation::DatabaseClose => {
            let (directory, file, records) = (
                table.directory.clone(),
                table
                    .file
                    .clone()
                    .ok_or_else(|| format!("database {name} is not open"))?,
                table
                    .records
                    .clone()
                    .ok_or_else(|| format!("database {name} is not open"))?,
            );
            host.save_database(&directory, &file, &records)
                .map_err(|cause| format!("FechaBD save: {cause}"))?;
            host.close_database(&directory, &file)
                .map_err(|cause| format!("FechaBD: {cause}"))?;
            let table = databases
                .get_mut(&name)
                .expect("database remains registered while closing");
            table.file = None;
            table.records = None;
        }
        Operation::DatabaseAppend => {
            let (directory, file, records) = {
                let table = databases
                    .get_mut(&name)
                    .expect("database remains registered while appending");
                let records = table
                    .records
                    .as_mut()
                    .ok_or_else(|| format!("database {name} is not open"))?;
                records.push(Some(build_record(&table.fields, &table.current)));
                table.current = blank_record(&table.fields);
                (
                    table.directory.clone(),
                    table.file.clone().expect("open database has a filename"),
                    records.clone(),
                )
            };
            host.save_database(&directory, &file, &records)
                .map_err(|cause| format!("AdicionaRegistro: {cause}"))?;
        }
        Operation::DatabaseRead => {
            let index = arguments
                .get(1)
                .ok_or("LeRegistro requires a record position")?
                .number()
                .map_err(|cause| format!("LeRegistro: {cause}"))?;
            if index < 0.0 || index.fract() != 0.0 || index > usize::MAX as f64 {
                return Err("LeRegistro position must be a nonnegative integer".into());
            }
            let records = table
                .records
                .as_ref()
                .ok_or_else(|| format!("database {name} is not open"))?;
            let physical = database_view(table)
                .get(index as usize)
                .copied()
                .ok_or_else(|| format!("record position {index} is out of range"))?;
            let record = records
                .get(physical)
                .cloned()
                .ok_or_else(|| format!("record position {index} is out of range"))?;
            let table = databases
                .get_mut(&name)
                .expect("database remains registered while reading");
            table.current = record.unwrap_or_else(|| blank_record(&table.fields));
        }
        Operation::DatabaseDelete => {
            let index = arguments
                .get(1)
                .ok_or("ExcluiRegistro requires a record position")?
                .number()
                .map_err(|cause| format!("ExcluiRegistro: {cause}"))?;
            if index < 0.0 || index.fract() != 0.0 || index > usize::MAX as f64 {
                return Err("ExcluiRegistro position must be a nonnegative integer".into());
            }
            if table.records.is_none() {
                return Err(format!("database {name} is not open"));
            }
            let physical = database_view(table)
                .get(index as usize)
                .copied()
                .ok_or_else(|| format!("record position {index} is out of range"))?;
            let (directory, file, records) = {
                let table = databases
                    .get_mut(&name)
                    .expect("database remains registered while deleting");
                let records = table
                    .records
                    .as_mut()
                    .ok_or_else(|| format!("database {name} is not open"))?;
                if records[physical].is_none() {
                    return Err(format!("record position {index} is already deleted"));
                }
                records[physical] = None;
                (
                    table.directory.clone(),
                    table.file.clone().expect("open database has a filename"),
                    records.clone(),
                )
            };
            host.save_database(&directory, &file, &records)
                .map_err(|cause| format!("ExcluiRegistro: {cause}"))?;
        }
        Operation::DatabaseOrder => {
            let order = arguments
                .iter()
                .skip(1)
                .map(Value::text)
                .collect::<Vec<_>>();
            let table = databases
                .get_mut(&name)
                .expect("database remains registered while ordering");
            if table.records.is_none() {
                return Err(format!("database {name} is not open"));
            }
            if order
                .first()
                .is_some_and(|field| field.eq_ignore_ascii_case("SEQUENCIAL"))
            {
                table.order = None;
            } else if order.is_empty() {
                return Err("OrdemBD requires at least one field or Sequencial".into());
            } else {
                for field in &order {
                    if !table
                        .fields
                        .iter()
                        .any(|(defined, _)| defined.eq_ignore_ascii_case(field))
                    {
                        return Err(format!("unknown field {name}.{field}"));
                    }
                }
                table.order = Some(order);
            }
        }
        Operation::DatabaseCompact => {
            let (directory, file, records) = {
                let table = databases
                    .get_mut(&name)
                    .expect("database remains registered while compacting");
                let records = table
                    .records
                    .as_mut()
                    .ok_or_else(|| format!("database {name} is not open"))?;
                records.retain(Option::is_some);
                (
                    table.directory.clone(),
                    table.file.clone().expect("open database has a filename"),
                    records.clone(),
                )
            };
            host.save_database(&directory, &file, &records)
                .map_err(|cause| format!("LimpaBD: {cause}"))?;
        }
        _ => return Err(format!("unsupported database operation {operation:?}")),
    }
    let _ = window;
    Ok(())
}

fn database_view(table: &Database) -> Vec<usize> {
    let Some(records) = table.records.as_ref() else {
        return Vec::new();
    };
    let mut positions: Vec<_> = (0..records.len()).collect();
    if let Some(fields) = &table.order {
        positions.sort_by(|left, right| match (&records[*left], &records[*right]) {
            (Some(left), Some(right)) => compare_records(left, right, fields),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        });
    }
    positions
}

fn compare_records(left: &Record, right: &Record, fields: &[String]) -> Ordering {
    fields
        .iter()
        .map(|field| {
            let left = left
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(field))
                .map(|(_, value)| value);
            let right = right
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(field))
                .map(|(_, value)| value);
            compare_database_values(left, right)
        })
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

fn compare_database_values(left: Option<&HostValue>, right: Option<&HostValue>) -> Ordering {
    match (left, right) {
        (Some(HostValue::Number(left)), Some(HostValue::Number(right))) => {
            left.partial_cmp(right).unwrap_or(Ordering::Equal)
        }
        (Some(left), Some(right)) => left.text().cmp(&right.text()),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
    }
}

fn find_database_record(table: &Database, mode: &str, search: &[Value]) -> Result<f64, String> {
    let fields = table
        .order
        .as_ref()
        .ok_or_else(|| "Procure requires an active OrdemBD index".to_owned())?;
    if search.is_empty() || search.len() > fields.len() {
        return Err("Procure needs search values matching the active index".into());
    }
    let records = table
        .records
        .as_ref()
        .ok_or_else(|| "database is not open".to_owned())?;
    let view = database_view(table);
    let matches = view
        .iter()
        .enumerate()
        .filter_map(|(position, physical)| {
            let record = records[*physical].as_ref()?;
            let exact = search.iter().enumerate().all(|(index, target)| {
                let field = &fields[index];
                let value = record
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(field))
                    .map(|(_, value)| value);
                compare_database_values(value, Some(&to_host_value(target.clone())))
                    == Ordering::Equal
            });
            exact.then_some(position)
        })
        .collect::<Vec<_>>();
    match mode.to_ascii_uppercase().as_str() {
        "PRIMEIRO" => Ok(matches.first().map_or(-1.0, |position| *position as f64)),
        "ULTIMO" => Ok(matches.last().map_or(-1.0, |position| *position as f64)),
        "PROXIMO" => {
            let record = view
                .iter()
                .enumerate()
                .filter_map(|(position, physical)| {
                    let row = records[*physical].as_ref()?;
                    let ordering = search
                        .iter()
                        .enumerate()
                        .map(|(index, target)| {
                            let field = &fields[index];
                            let value = row
                                .iter()
                                .find(|(name, _)| name.eq_ignore_ascii_case(field))
                                .map(|(_, value)| value);
                            compare_database_values(value, Some(&to_host_value(target.clone())))
                        })
                        .find(|ordering| *ordering != Ordering::Equal)
                        .unwrap_or(Ordering::Equal);
                    (ordering != Ordering::Less).then_some(position)
                })
                .next();
            Ok(record.map_or(-1.0, |position| position as f64))
        }
        _ => Err("Procure expects Primeiro, Ultimo, or Proximo".into()),
    }
}

fn to_host_value(value: Value) -> HostValue {
    match value {
        Value::Text(text) => HostValue::Text(text),
        Value::Number(number) => HostValue::Number(number),
        Value::Logic(value) => HostValue::Logic(value),
        Value::Object(key) => HostValue::Object(key),
        Value::Sound(key) => HostValue::Sound(key),
        Value::Color(r, g, b) => HostValue::Color(r, g, b),
        Value::Array(values) => HostValue::Array(
            values
                .into_iter()
                .map(|value| value.map(to_host_value))
                .collect(),
        ),
    }
}

fn from_host_value(value: HostValue) -> Value {
    match value {
        HostValue::Text(text) => Value::Text(text),
        HostValue::Object(key) => Value::Object(key),
        HostValue::Sound(key) => Value::Sound(key),
        HostValue::Number(number) => Value::Number(number),
        HostValue::Logic(value) => Value::Logic(value),
        HostValue::Color(r, g, b) => Value::Color(r, g, b),
        HostValue::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| value.map(from_host_value))
                .collect(),
        ),
    }
}

fn to_host_memory(value: Value) -> HostMemory {
    match value {
        Value::Array(values) => HostMemory::Array(
            values
                .into_iter()
                .map(|value| value.map(to_host_value))
                .collect(),
        ),
        value => HostMemory::Scalar(to_host_value(value)),
    }
}

fn from_host_memory(value: HostMemory) -> Value {
    match value {
        HostMemory::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| value.map(from_host_value))
                .collect(),
        ),
        HostMemory::Scalar(value) => from_host_value(value),
    }
}

struct CallbackHost<F>(F);

impl<F> Host for CallbackHost<F>
where
    F: FnMut(&str, &Window) -> io::Result<String>,
{
    fn request(&mut self, request: Request<'_>) -> io::Result<Option<HostValue>> {
        if request.operation != Operation::InputText {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("{:?} is unavailable", request.operation),
            ));
        }
        let prompt = request
            .arguments
            .first()
            .map(|argument| argument.value.text())
            .unwrap_or_default();
        (self.0)(&prompt, request.window).map(|text| Some(HostValue::Text(text)))
    }
}

type Memories = HashMap<String, Value>;
const MAX_ARRAY_ELEMENTS: usize = 1_000_000;

fn evaluate_operand(
    operand: &Operand,
    memories: &Memories,
    databases: &Databases,
    host: &mut dyn Host,
    window: &Window,
) -> Result<Value, String> {
    match &operand.value {
        syntax::Atom::Literal(value) => Ok(value.clone()),
        syntax::Atom::Memory(name) => memories
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unknown memory {name}")),
        syntax::Atom::DatabaseField(database, field) => databases
            .get(database)
            .ok_or_else(|| format!("unknown database {database}"))?
            .current
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(field))
            .map(|(_, value)| from_host_value(value.clone()))
            .ok_or_else(|| format!("unknown field {database}.{field}")),
        syntax::Atom::Reserved(name) => {
            let argument = HostArgument {
                name: None,
                value: HostValue::Text(name.clone()),
            };
            host.request(Request {
                operation: Operation::ReservedMemory,
                arguments: &[argument],
                rows: &[],
                window,
            })
            .map_err(|cause| format!("reserved memory {name}: {cause}"))?
            .map(from_host_value)
            .ok_or_else(|| format!("host did not return reserved memory {name}"))
        }
        syntax::Atom::Builtin(name, arguments) => {
            evaluate_builtin(name, arguments, memories, databases, host, window)
        }
        syntax::Atom::IndexedMemory(name, index) => {
            let index = evaluate(index, memories, databases, host, window)?.index()?;
            match memories.get(name) {
                Some(Value::Array(items)) => items
                    .get(index)
                    .ok_or_else(|| format!("array index {index} is out of range"))?
                    .clone()
                    .ok_or_else(|| format!("array index {index} has not been assigned")),
                Some(_) => Err(format!("{name} is not an array memory")),
                None => Err(format!("unknown array memory {name}")),
            }
        }
    }
}

fn evaluate(
    expression: &Expression,
    memories: &Memories,
    databases: &Databases,
    host: &mut dyn Host,
    window: &Window,
) -> Result<Value, String> {
    fn evaluate_product(
        product: &syntax::Product,
        memories: &Memories,
        databases: &Databases,
        host: &mut dyn Host,
        window: &Window,
    ) -> Result<Value, String> {
        let mut value = evaluate_operand(&product.first, memories, databases, host, window)?;
        for (operator, operand) in &product.rest {
            value = calculate(
                &value,
                *operator,
                &evaluate_operand(operand, memories, databases, host, window)?,
            )?;
        }
        Ok(value)
    }

    let mut value = evaluate_product(&expression.first, memories, databases, host, window)?;
    for (operator, product) in &expression.rest {
        value = calculate(
            &value,
            *operator,
            &evaluate_product(product, memories, databases, host, window)?,
        )?;
    }
    Ok(value)
}

fn evaluate_builtin(
    name: &str,
    arguments: &[Operand],
    memories: &Memories,
    databases: &Databases,
    host: &mut dyn Host,
    window: &Window,
) -> Result<Value, String> {
    let values = arguments
        .iter()
        .map(|argument| evaluate_operand(argument, memories, databases, host, window))
        .collect::<Result<Vec<_>, _>>()?;
    match name {
        "UNIR" => Ok(Value::Text(values.iter().map(Value::text).collect())),
        "SEPARE" => {
            if !(2..=3).contains(&values.len()) {
                return Err("Separe expects text, start position, and optional length".into());
            }
            let characters: Vec<_> = values[0].text().chars().collect();
            let start = integer_value(&values[1], "Separe start")?;
            if start < 1 {
                return Err("Separe positions start at 1".into());
            }
            let start = usize::try_from(start - 1)
                .map_err(|_| "Separe start is out of range".to_owned())?;
            let length = values
                .get(2)
                .map(|value| integer_value(value, "Separe length"))
                .transpose()?
                .unwrap_or_else(|| characters.len() as i64);
            if length < 0 {
                return Err("Separe length cannot be negative".into());
            }
            let length =
                usize::try_from(length).map_err(|_| "Separe length is out of range".to_owned())?;
            let text = characters.into_iter().skip(start).take(length).collect();
            Ok(Value::Text(text))
        }
        "ALINHA" => {
            if values.len() != 3 {
                return Err("Alinha expects text, field width, and DIR or ESQ".into());
            }
            let text = values[0].text();
            let width = integer_value(&values[1], "Alinha width")?;
            if !(0..=1_000_000).contains(&width) {
                return Err("Alinha width must be from 0 to 1000000".into());
            }
            let direction = values[2].text();
            if !direction.eq_ignore_ascii_case("DIR") && !direction.eq_ignore_ascii_case("ESQ") {
                return Err("Alinha direction must be DIR or ESQ".into());
            }
            let width = width as usize;
            let mut characters: Vec<_> = text.chars().take(width).collect();
            let padding = width.saturating_sub(characters.len());
            if direction.eq_ignore_ascii_case("DIR") {
                let mut aligned = vec![' '; padding];
                aligned.append(&mut characters);
                Ok(Value::Text(aligned.into_iter().collect()))
            } else {
                characters.extend(std::iter::repeat_n(' ', padding));
                Ok(Value::Text(characters.into_iter().collect()))
            }
        }
        "FORMATA" => {
            if values.len() != 2 {
                return Err("Formata expects a number and a format mask".into());
            }
            Ok(Value::Text(format_number(&values[0], &values[1].text())?))
        }
        "RGB" => {
            if values.len() != 3 {
                return Err("RGB expects three color components".into());
            }
            let components = values
                .iter()
                .map(|value| {
                    let value = integer_value(value, "RGB component")?;
                    u8::try_from(value).map_err(|_| "RGB components must be from 0 to 255".into())
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Value::Color(components[0], components[1], components[2]))
        }
        "TOTALREGISTROS" => {
            if values.len() != 1 {
                return Err("TotalRegistros expects a database name".into());
            }
            let database = databases
                .get(&values[0].text().to_ascii_uppercase())
                .ok_or_else(|| format!("unknown database {}", values[0].text()))?;
            let count = database
                .records
                .as_ref()
                .ok_or_else(|| format!("database {} is not open", values[0].text()))?
                .len();
            Ok(Value::Number(count as f64))
        }
        "PROCURE" => {
            if values.len() < 3 {
                return Err("Procure expects a database, search mode, and value".into());
            }
            let database = databases
                .get(&values[0].text().to_ascii_uppercase())
                .ok_or_else(|| format!("unknown database {}", values[0].text()))?;
            Ok(Value::Number(find_database_record(
                database,
                &values[1].text(),
                &values[2..],
            )?))
        }
        "ALEATORIO" => {
            if values.len() != 1 {
                return Err("Aleatorio expects an inclusive maximum".into());
            }
            let maximum = values[0].number()?;
            if maximum < 0.0 || maximum.fract() != 0.0 {
                return Err("Aleatorio maximum must be a nonnegative integer".into());
            }
            let arguments = [HostArgument {
                name: None,
                value: to_host_value(values[0].clone()),
            }];
            let result = host
                .request(Request {
                    operation: Operation::Random,
                    arguments: &arguments,
                    rows: &[],
                    window,
                })
                .map_err(|cause| format!("Aleatorio: {cause}"))?
                .ok_or_else(|| "Aleatorio did not return a value".to_owned())?;
            let HostValue::Number(number) = result else {
                return Err("Aleatorio must return a number".into());
            };
            if number.fract() != 0.0 || number < 0.0 || number > maximum {
                return Err("Aleatorio result is outside the requested range".into());
            }
            Ok(Value::Number(number))
        }
        "DATA" | "HORA" | "TESTAMOUSE" => {
            let operation = match name {
                "DATA" => Operation::Date,
                "HORA" => Operation::Time,
                "TESTAMOUSE" => Operation::MouseButtons,
                _ => unreachable!("only date, time, and mouse remain host-backed"),
            };
            let host_arguments: Vec<_> = values
                .into_iter()
                .map(|value| HostArgument {
                    name: None,
                    value: to_host_value(value),
                })
                .collect();
            host.request(Request {
                operation,
                arguments: &host_arguments,
                rows: &[],
                window,
            })
            .map_err(|cause| format!("{name}: {cause}"))?
            .map(from_host_value)
            .ok_or_else(|| format!("{name} did not return a value"))
        }
        _ => Err(format!("unsupported function {name}")),
    }
}

fn integer_value(value: &Value, description: &str) -> Result<i64, String> {
    let number = value.number()?;
    if number.fract() != 0.0 || number < i64::MIN as f64 || number > i64::MAX as f64 {
        return Err(format!("{description} must be an integer"));
    }
    Ok(number as i64)
}

fn format_number(value: &Value, mask: &str) -> Result<String, String> {
    let number = match value {
        Value::Number(value) => *value,
        Value::Text(value) => value
            .replace('.', "")
            .replace(',', ".")
            .parse::<f64>()
            .map_err(|_| "Formata expects a numeric value".to_owned())?,
        _ => return Err("Formata expects a numeric value".into()),
    };
    if !number.is_finite() {
        return Err("Formata expects a finite number".into());
    }
    let normalized = if mask.eq_ignore_ascii_case("PADRAO") {
        "#.##0,00"
    } else {
        mask
    };
    let (integer_mask, decimal_mask) = normalized
        .split_once(',')
        .map_or((normalized, ""), |(integer, decimal)| (integer, decimal));
    let decimal_places = decimal_mask.chars().count();
    let raw = format!("{:.*}", decimal_places, number.abs());
    let (raw_integer, raw_decimal) = raw.split_once('.').unwrap_or((&raw, ""));
    let minimum_digits = integer_mask
        .chars()
        .filter(|character| *character == '0')
        .count();
    let mut integer = format!("{:0>width$}", raw_integer, width = minimum_digits);
    if integer_mask.contains('.') {
        let mut grouped = String::new();
        for (index, character) in integer.chars().rev().enumerate() {
            if index > 0 && index % 3 == 0 {
                grouped.push('.');
            }
            grouped.push(character);
        }
        integer = grouped.chars().rev().collect();
    }
    let mut result = if raw_decimal.is_empty() {
        integer
    } else {
        format!("{integer},{raw_decimal}")
    };
    let optional_decimal_places = decimal_mask
        .chars()
        .rev()
        .take_while(|character| *character == '#')
        .count();
    for _ in 0..optional_decimal_places {
        if result.ends_with('0') {
            result.pop();
        } else {
            break;
        }
    }
    if result.ends_with(',') {
        result.pop();
    }
    if number.is_sign_negative() {
        result.insert(0, '-');
    }
    Ok(result)
}

fn calculate(left: &Value, operator: Arithmetic, right: &Value) -> Result<Value, String> {
    let left = left.number()?;
    let right = right.number()?;
    let result = match operator {
        Arithmetic::Add => left + right,
        Arithmetic::Subtract => left - right,
        Arithmetic::Multiply => left * right,
        Arithmetic::Divide | Arithmetic::Remainder if right == 0.0 => {
            return Err("division by zero".into());
        }
        Arithmetic::Divide => left / right,
        Arithmetic::Remainder => left % right,
    };
    if !result.is_finite() {
        return Err("numeric result is out of range".into());
    }
    Ok(Value::Number(result))
}

fn check(
    condition: &Condition,
    memories: &Memories,
    databases: &Databases,
    host: &mut dyn Host,
    window: &Window,
) -> Result<bool, String> {
    for conjunction in condition {
        let mut matches = true;
        for comparison in conjunction {
            let left = evaluate_operand(&comparison.left, memories, databases, host, window)?;
            let right = evaluate_operand(&comparison.right, memories, databases, host, window)?;
            let ordering = match (&left, &right) {
                (Value::Number(left), Value::Number(right)) => left.partial_cmp(right),
                (Value::Text(left), Value::Text(right)) => Some(left.cmp(right)),
                (Value::Logic(left), Value::Logic(right)) => Some(left.cmp(right)),
                (Value::Object(left), Value::Object(right))
                | (Value::Sound(left), Value::Sound(right)) => Some(left.cmp(right)),
                (Value::Color(lr, lg, lb), Value::Color(rr, rg, rb)) => {
                    Some((lr, lg, lb).cmp(&(rr, rg, rb)))
                }
                (Value::Number(left), Value::Text(_)) => comparison
                    .right
                    .decimal
                    .and_then(|right| left.partial_cmp(&right)),
                (Value::Text(_), Value::Number(right)) => comparison
                    .left
                    .decimal
                    .and_then(|left| left.partial_cmp(right)),
                _ => None,
            };
            matches &= match (comparison.operator, ordering) {
                (Comparison::Equal, ordering) => ordering == Some(Ordering::Equal),
                (Comparison::NotEqual, ordering) => ordering != Some(Ordering::Equal),
                (_, None) => return Err("cannot order values of different types".into()),
                (Comparison::Less, Some(ordering)) => ordering.is_lt(),
                (Comparison::LessOrEqual, Some(ordering)) => ordering.is_le(),
                (Comparison::Greater, Some(ordering)) => ordering.is_gt(),
                (Comparison::GreaterOrEqual, Some(ordering)) => ordering.is_ge(),
            };
        }
        if matches {
            return Ok(true);
        }
    }
    Ok(false)
}
