use std::collections::BTreeMap;
use std::io;

use crate::Window;

/// Values are passed by value across the host boundary. Resource keys are
/// returned by the host when an object or sound is created.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Number(f64),
    Logic(bool),
    Object(String),
    Sound(String),
    Color(u8, u8, u8),
    Array(Vec<Option<Value>>),
}

impl Value {
    pub fn number(&self) -> Result<f64, String> {
        match self {
            Self::Number(number) if number.is_finite() => Ok(*number),
            _ => Err("expected a finite numeric value".into()),
        }
    }

    pub fn text(&self) -> String {
        match self {
            Self::Text(text) | Self::Object(text) | Self::Sound(text) => text.clone(),
            Self::Number(number) => number.to_string().replace('.', ","),
            Self::Logic(true) => "SIM".into(),
            Self::Logic(false) => "NAO".into(),
            Self::Color(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
            Self::Array(values) => format!("[array of {} positions]", values.len()),
        }
    }

    pub fn index(&self) -> Result<usize, String> {
        let number = self.number()?;
        if number < 0.0 || number.fract() != 0.0 || number >= usize::MAX as f64 {
            return Err("expected a nonnegative integer".into());
        }
        Ok(number as usize)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Memory {
    Scalar(Value),
    Array(Vec<Option<Value>>),
}

pub type Snapshot = BTreeMap<String, Memory>;
pub type Record = BTreeMap<String, Value>;

/// Deleted records stay vacant until LimpaBD compacts the file.
pub type Records = Vec<Option<Record>>;

#[derive(Clone, Debug)]
pub struct Argument {
    pub name: Option<String>,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    InputText,
    Message,
    WaitKey,
    WaitSeconds,
    WaitObject,
    WaitButton,
    MouseButtons,
    ReservedMemory,
    Date,
    Time,
    Random,
    Background,
    Font,
    ConfigureWindow,
    CreateObject,
    CreateSound,
    Draw,
    DrawObjects,
    PositionObject,
    Transparency,
    Play,
    StopSound,
    Video,
    Component,
    Menu,
    Buttons,
    RemoveButtons,
    BoxPosition,
    BoxConfig,
    BoxTitle,
    BoxActivate,
    BoxAdd,
    BoxNew,
    BoxDeactivate,
    PrintPosition,
    PrintWrite,
    PrintDraw,
    PrintFont,
    PrintNewPage,
    PrintFinish,
    DatabaseOpen,
    DatabaseDefine,
    DatabaseClose,
    DatabaseAppend,
    DatabaseRead,
    DatabaseDelete,
    DatabaseOrder,
    DatabaseFind,
    DatabaseCount,
    DatabaseCompact,
}

/// Arguments retain source order. Menu and button entries have separate rows.
/// Object and sound creation receive the memory name followed by the path.
pub struct Request<'a> {
    pub operation: Operation,
    pub arguments: &'a [Argument],
    pub rows: &'a [Vec<Argument>],
    pub window: &'a Window,
}

/// Implement the capabilities available in the embedding environment.
/// Unsupported operations return a source-located execution error.
pub trait Host {
    fn request(&mut self, request: Request<'_>) -> io::Result<Option<Value>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("{:?} is unavailable", request.operation),
        ))
    }

    /// Called after changes to the text grid or its cursor.
    fn window_changed(&mut self, _window: &Window) -> io::Result<()> {
        Ok(())
    }

    /// Called before each source statement so an embedding environment can
    /// process cancellation, pause, or single-step controls.
    fn checkpoint(&mut self, _line: usize) -> io::Result<()> {
        Ok(())
    }

    fn save_memories(&mut self, _path: &str, _snapshot: &Snapshot) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    /// Return None for a missing file.
    fn load_memories(&mut self, _path: &str) -> io::Result<Option<Snapshot>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn load_program(&mut self, _path: &str) -> io::Result<String> {
        Err(io::ErrorKind::Unsupported.into())
    }

    /// Return an empty collection for a new database. The host supplies the
    /// storage format and resolves the directory relative to the program.
    fn open_database(
        &mut self,
        _directory: &str,
        _file: &str,
        _fields: &[(String, usize)],
    ) -> io::Result<Records> {
        Err(io::ErrorKind::Unsupported.into())
    }

    /// Persist the entire logical table atomically, including deleted slots.
    fn save_database(
        &mut self,
        _directory: &str,
        _file: &str,
        _records: &Records,
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn close_database(&mut self, _directory: &str, _file: &str) -> io::Result<()> {
        Ok(())
    }
}
