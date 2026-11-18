use std::collections::{HashMap, HashSet};

use crate::{Error, MAX_ARRAY_ELEMENTS, Operation};

pub(crate) struct Statement {
    pub(crate) line: usize,
    pub(crate) command: Command,
}

pub(crate) enum Command {
    Declare(Vec<(String, Option<usize>)>),
    Assign(String, Expression),
    ArrayAssign(String, Expression, Expression),
    FindArray(String, String, Operand),
    Update(String, Arithmetic, Expression),
    ReadText(String, Operand),
    Position(Operand, Operand),
    Write(Vec<Operand>),
    Clear,
    Message(Vec<Operand>),
    Wait(Vec<Operand>),
    Host(Operation, Vec<Operand>),
    HostAssign(String, Operation, Vec<Operand>),
    HostBlock(Operation, Vec<Operand>, Vec<Vec<Operand>>),
    CreateResources(Operation, Vec<(String, Operand)>),
    Database(Operation, Vec<Operand>),
    DatabaseFieldAssign(String, String, Expression),
    SaveMemories(Operand),
    LoadMemories(Operand),
    LoadProgram(Operand),
    RemoveMemory(String),
    ReorganizeMemories,
    If(Condition, usize),
    Jump(usize),
    Call(usize),
    RoutineStart,
    Return,
    EndProgram,
    EndWindow,
}

#[derive(Clone, PartialEq)]
pub(crate) enum Value {
    Text(String),
    Number(f64),
    Logic(bool),
    Object(String),
    Sound(String),
    Color(u8, u8, u8),
    Array(Vec<Option<Value>>),
}

impl Value {
    pub(crate) fn number(&self) -> Result<f64, String> {
        match self {
            Self::Number(number) => Ok(*number),
            _ => Err("expected a numeric value".into()),
        }
    }

    pub(crate) fn text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Number(number) => number.to_string().replace('.', ","),
            Self::Logic(true) => "SIM".into(),
            Self::Logic(false) => "NAO".into(),
            Self::Object(key) | Self::Sound(key) => key.clone(),
            Self::Color(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
            Self::Array(_) => "[matriz]".into(),
        }
    }

    pub(crate) fn index(&self) -> Result<usize, String> {
        let number = self.number()?;
        if number < 0.0 || number.fract() != 0.0 || number > usize::MAX as f64 {
            return Err("array index must be a nonnegative integer".into());
        }
        Ok(number as usize)
    }
}

pub(crate) enum Atom {
    Literal(Value),
    Memory(String),
    IndexedMemory(String, Box<Expression>),
    DatabaseField(String, String),
    Reserved(String),
    Builtin(String, Vec<Operand>),
}

pub(crate) struct Operand {
    pub(crate) value: Atom,
    pub(crate) decimal: Option<f64>,
    pub(crate) name: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) enum Arithmetic {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

pub(crate) struct Product {
    pub(crate) first: Operand,
    pub(crate) rest: Vec<(Arithmetic, Operand)>,
}

pub(crate) struct Expression {
    pub(crate) first: Product,
    pub(crate) rest: Vec<(Arithmetic, Product)>,
}

#[derive(Clone, Copy)]
pub(crate) enum Comparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

pub(crate) struct Predicate {
    pub(crate) left: Operand,
    pub(crate) operator: Comparison,
    pub(crate) right: Operand,
}

// Disjunction of conjunctions, with E binding more tightly than OU.
pub(crate) type Condition = Vec<Vec<Predicate>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Token<'source> {
    Name(&'source str),
    Digits(&'source str),
    Text(&'source str),
    Symbol(&'source str),
}

fn tokenize(source: &str) -> Result<Vec<(usize, Vec<Token<'_>>)>, Error> {
    let mut commands = Vec::new();
    for (index, mut text) in source.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = index + 1;
        let mut tokens = Vec::new();
        loop {
            text = text.trim_start();
            let Some(character) = text.chars().next() else {
                break;
            };
            match character {
                '\'' => break,
                ';' => {
                    if !tokens.is_empty() {
                        commands.push((line, std::mem::take(&mut tokens)));
                    }
                    text = &text[1..];
                }
                '"' => {
                    let Some(end) = text[1..].find('"') else {
                        return Err(Error::new(line, "unterminated quoted text".into()));
                    };
                    tokens.push(Token::Text(&text[1..end + 1]));
                    text = &text[end + 2..];
                }
                first if first.is_ascii_alphabetic() || first == '_' => {
                    let end = text
                        .find(|character: char| {
                            !character.is_ascii_alphanumeric() && character != '_'
                        })
                        .unwrap_or(text.len());
                    tokens.push(Token::Name(&text[..end]));
                    text = &text[end..];
                }
                first if first.is_ascii_digit() => {
                    let end = text
                        .find(|character: char| !character.is_ascii_digit())
                        .unwrap_or(text.len());
                    tokens.push(Token::Digits(&text[..end]));
                    text = &text[end..];
                }
                _ => {
                    let symbol = ["++", "--", "+=", "-=", "<=", ">=", "<>"]
                        .into_iter()
                        .find(|operator| text.starts_with(operator))
                        .unwrap_or(&text[..character.len_utf8()]);
                    tokens.push(Token::Symbol(symbol));
                    text = &text[symbol.len()..];
                }
            }
        }
        if !tokens.is_empty() {
            commands.push((line, tokens));
        }
    }
    Ok(commands)
}

pub(crate) fn parse(source: &str) -> Result<Vec<Statement>, Error> {
    let commands = tokenize(source)?;
    let destinations: HashSet<_> = commands
        .iter()
        .filter_map(|(_, tokens)| match tokens.as_slice() {
            [Token::Name(command), Token::Name(label)]
                if command.eq_ignore_ascii_case("VaPara") =>
            {
                Some(label.to_ascii_uppercase())
            }
            _ => None,
        })
        .collect();
    let mut statements: Vec<Statement> = Vec::new();
    let mut conditions = Vec::new();
    let mut jumps = Vec::new();
    let mut calls = Vec::new();
    let mut labels = HashMap::new();
    let mut routines = HashMap::new();
    let mut memories = HashSet::new();
    let mut consumed = HashSet::new();
    let mut declaring = true;
    let mut header = true;
    let mut main_ended = false;

    for (command_index, (line, tokens)) in commands.iter().enumerate() {
        if consumed.contains(&command_index) {
            continue;
        }
        let line = *line;
        let error = |message| Error::new(line, message);
        let mut parser = Parser { tokens };
        let name = parser.name().map_err(error)?;
        if name == "BD" && !header {
            return Err(error("BD definitions belong in the program header".into()));
        }
        let command = match name.as_str() {
            "MEMORIA" => {
                if !declaring {
                    return Err(error(
                        "Memoria declarations belong at the start of the program".into(),
                    ));
                }
                let mut names = Vec::new();
                loop {
                    let name = parser.name().map_err(error)?;
                    let size = if parser.take(Token::Symbol("(")) {
                        let size = match parser.next() {
                            Some(Token::Digits(value)) => value
                                .parse::<usize>()
                                .ok()
                                .filter(|value| (1..=MAX_ARRAY_ELEMENTS).contains(value))
                                .ok_or_else(|| error("invalid array size".into()))?,
                            _ => return Err(error("expected array size".into())),
                        };
                        if !parser.take(Token::Symbol(")")) {
                            return Err(error("expected ) after array size".into()));
                        }
                        Some(size)
                    } else {
                        None
                    };
                    if reserved(&name) || !memories.insert(name.clone()) {
                        return Err(error(format!("cannot declare memory {name}")));
                    }
                    names.push((name, size));
                    if !parser.take(Token::Symbol(",")) {
                        break;
                    }
                }
                Command::Declare(names)
            }
            "POSICIONE" => {
                let row = parser
                    .operand(OperandContext::Argument, &memories)
                    .map_err(error)?;
                parser.comma().map_err(error)?;
                let column = parser
                    .operand(OperandContext::Argument, &memories)
                    .map_err(error)?;
                Command::Position(row, column)
            }
            "ESCREVA" => {
                let mut items = Vec::new();
                loop {
                    items.push(
                        parser
                            .operand(OperandContext::Output, &memories)
                            .map_err(error)?,
                    );
                    if !parser.take(Token::Symbol(",")) {
                        break;
                    }
                }
                Command::Write(items)
            }
            "LIMPAJANELA" => Command::Clear,
            "MENSAGEM" => {
                let arguments = parser
                    .arguments(OperandContext::Output, &memories)
                    .map_err(error)?;
                if arguments.is_empty() {
                    return Err(error("Mensagem requires at least one value".into()));
                }
                Command::Message(arguments)
            }
            "AGUARDE" => {
                let arguments = if parser.tokens.is_empty() {
                    Vec::new()
                } else {
                    vec![
                        parser
                            .operand(OperandContext::Wait, &memories)
                            .map_err(error)?,
                    ]
                };
                Command::Wait(arguments)
            }
            "COMPONENTE" if parser.keyword("Valor") => {
                parser.take(Token::Symbol(","));
                let component = parser
                    .operand(OperandContext::Argument, &memories)
                    .map_err(error)?;
                if !parser.tokens.is_empty() {
                    return Err(error(
                        "unexpected arguments after component identifier".into(),
                    ));
                }
                let arguments = vec![
                    Operand {
                        value: Atom::Literal(Value::Text("Valor".into())),
                        decimal: None,
                        name: None,
                    },
                    component,
                ];
                Command::HostAssign("RETORNO".into(), Operation::Component, arguments)
            }
            "CRIA" => {
                let operation = match parser.name().map_err(error)?.as_str() {
                    "OBJETO" => Operation::CreateObject,
                    "SOM" => Operation::CreateSound,
                    kind => return Err(error(format!("unsupported Cria resource kind {kind}"))),
                };
                parser.end().map_err(error)?;
                let close_index = commands
                    .iter()
                    .enumerate()
                    .skip(command_index + 1)
                    .find_map(|(index, (_, row))| match row.as_slice() {
                        [Token::Name(end)] if end.eq_ignore_ascii_case("FimCria") => Some(index),
                        _ => None,
                    })
                    .ok_or_else(|| error("FimCria is missing".into()))?;
                let mut resources = Vec::new();
                for (row_index, (_, row_tokens)) in commands
                    .iter()
                    .enumerate()
                    .take(close_index)
                    .skip(command_index + 1)
                {
                    let mut row_parser = Parser { tokens: row_tokens };
                    let memory = row_parser.name().map_err(error)?;
                    if !memories.contains(&memory) {
                        return Err(error(format!("unknown resource memory {memory}")));
                    }
                    if !row_parser.take(Token::Symbol("=")) {
                        return Err(error("resource creation expects memory = path".into()));
                    }
                    let path = row_parser
                        .operand(OperandContext::Argument, &memories)
                        .map_err(error)?;
                    row_parser.end().map_err(error)?;
                    resources.push((memory, path));
                    consumed.insert(row_index);
                }
                if resources.is_empty() {
                    return Err(error("Cria requires at least one resource".into()));
                }
                consumed.insert(close_index);
                Command::CreateResources(operation, resources)
            }
            "IMP" => {
                let operation = match parser.name().map_err(error)?.as_str() {
                    "POSICIONE" => Operation::PrintPosition,
                    "ESCREVA" => Operation::PrintWrite,
                    "DESENHE" => Operation::PrintDraw,
                    "FONTE" => Operation::PrintFont,
                    "NOVAPAGINA" => Operation::PrintNewPage,
                    "FIM" => Operation::PrintFinish,
                    command => {
                        return Err(error(format!("unsupported printer command {command}")));
                    }
                };
                let arguments = parser
                    .arguments(OperandContext::Output, &memories)
                    .map_err(error)?;
                validate_host_argument_count(operation, arguments.len()).map_err(error)?;
                Command::Host(operation, arguments)
            }
            "BOTAO" if parser.keyword("Remove") => {
                Command::Host(Operation::RemoveButtons, Vec::new())
            }
            "MENU" | "BOTAO" | "BD" => {
                let (operation, close) = match name.as_str() {
                    "MENU" => (Operation::Menu, "FIMMENU"),
                    "BOTAO" => (Operation::Buttons, "FIMBOTAO"),
                    _ => (Operation::DatabaseDefine, "FIMBD"),
                };
                let header = if operation == Operation::DatabaseDefine {
                    parser.arguments_with_whitespace(OperandContext::Output, &memories)
                } else {
                    parser.arguments(OperandContext::Output, &memories)
                }
                .map_err(error)?;
                let close_index = commands
                    .iter()
                    .enumerate()
                    .skip(command_index + 1)
                    .find_map(|(index, (_, row))| match row.as_slice() {
                        [Token::Name(end)] if end.eq_ignore_ascii_case(close) => Some(index),
                        _ => None,
                    })
                    .ok_or_else(|| error(format!("{close} is missing")))?;
                let mut rows = Vec::new();
                for (row_index, (_, row_tokens)) in commands
                    .iter()
                    .enumerate()
                    .take(close_index)
                    .skip(command_index + 1)
                {
                    let mut row_parser = Parser { tokens: row_tokens };
                    let mut row = Vec::new();
                    while !row_parser.tokens.is_empty() {
                        row.push(
                            row_parser
                                .operand(OperandContext::Output, &memories)
                                .map_err(error)?,
                        );
                        if !row_parser.take(Token::Symbol(",")) {
                            break;
                        }
                    }
                    row_parser.end().map_err(error)?;
                    rows.push(row);
                    consumed.insert(row_index);
                }
                consumed.insert(close_index);
                match operation {
                    Operation::Menu => {
                        if header.len() != 1 {
                            return Err(error("Menu requires one heading".into()));
                        }
                        if rows.is_empty() || rows.len() > 10 {
                            return Err(error("Menu requires 1 to 10 choices".into()));
                        }
                        if rows.iter().any(|row| row.len() != 1) {
                            return Err(error("Menu choices each require one value".into()));
                        }
                    }
                    Operation::Buttons => {
                        let special = matches!(
                            header.as_slice(),
                            [Operand {
                                value: Atom::Literal(Value::Text(kind)),
                                ..
                            }] if kind.eq_ignore_ascii_case("ESPECIAL")
                        );
                        if (!header.is_empty() && !special) || rows.is_empty() {
                            return Err(error("Botao requires button definitions".into()));
                        }
                        if special && rows.iter().any(|row| row.len() != 4) {
                            return Err(error(
                                "Botao Especial definitions require a label, row, column, and type"
                                    .into(),
                            ));
                        }
                        if !special && rows.iter().any(|row| !(5..=6).contains(&row.len())) {
                            return Err(error(
                                "Botao definitions require a label, position, size, and optional image".into(),
                            ));
                        }
                    }
                    Operation::DatabaseDefine => {
                        if header.len() != 2 || rows.is_empty() {
                            return Err(error("BD requires a name, directory, and fields".into()));
                        }
                        if rows.iter().any(|row| row.len() != 2) {
                            return Err(error("BD field rows need a name and width".into()));
                        }
                    }
                    _ => unreachable!("only block operations are parsed here"),
                }
                Command::HostBlock(operation, header, rows)
            }
            "SE" => {
                let condition = parser.condition(&memories).map_err(error)?;
                conditions.push(statements.len());
                Command::If(condition, 0)
            }
            "CASOCONTRARIO" => {
                let index = statements.len();
                let previous = conditions.last_mut().ok_or_else(|| {
                    error("CasoContrario requires an open Se without an alternative".into())
                })?;
                let Command::If(_, otherwise) = &mut statements[*previous].command else {
                    return Err(error(
                        "CasoContrario requires an open Se without an alternative".into(),
                    ));
                };
                *otherwise = index + 1;
                *previous = index;
                Command::Jump(0)
            }
            "FIMSE" => {
                parser.end().map_err(error)?;
                let previous = conditions
                    .pop()
                    .ok_or_else(|| error("FimSe requires an open Se".into()))?;
                let destination = statements.len();
                match &mut statements[previous].command {
                    Command::If(_, target) | Command::Jump(target) => *target = destination,
                    _ => unreachable!("conditional stack records Se or CasoContrario"),
                }
                continue;
            }
            "VAPARA" => {
                jumps.push((statements.len(), parser.name().map_err(error)?));
                Command::Jump(0)
            }
            "EXECUTE" => {
                let routine = parser.name().map_err(error)?;
                if !parser.take(Token::Symbol("(")) || !parser.take(Token::Symbol(")")) {
                    return Err(error(
                        "Execute requires a routine name followed by ()".into(),
                    ));
                }
                calls.push((statements.len(), routine));
                Command::Call(0)
            }
            "RETORNE" => Command::Return,
            "FIMPROGRAMA" => Command::EndProgram,
            "FIMJANELA" => Command::EndWindow,
            "ABREBD" | "FECHABD" | "ADICIONAREGISTRO" | "LEREGISTRO" | "ORDEMBD"
            | "EXCLUIREGISTRO" | "LIMPABD" => {
                let operation = host_operation(&name).expect("database operation mapping");
                let arguments = parser
                    .arguments_with_whitespace(OperandContext::Output, &memories)
                    .map_err(error)?;
                validate_database_argument_count(operation, arguments.len()).map_err(error)?;
                Command::Database(operation, arguments)
            }
            "GRAVAMEMORIA" => Command::SaveMemories(
                parser
                    .operand(OperandContext::Argument, &memories)
                    .map_err(error)?,
            ),
            "CARREGAMEMORIA" => Command::LoadMemories(
                parser
                    .operand(OperandContext::Argument, &memories)
                    .map_err(error)?,
            ),
            "CARREGAPROGRAMA" => Command::LoadProgram(
                parser
                    .operand(OperandContext::Argument, &memories)
                    .map_err(error)?,
            ),
            "APAGAMEMORIA" => Command::RemoveMemory(parser.name().map_err(error)?),
            "REORGANIZAMEMORIA" => Command::ReorganizeMemories,
            _ if parser.take(Token::Symbol(".")) => {
                let field = parser.name().map_err(error)?;
                if !parser.take(Token::Symbol("=")) {
                    return Err(error("database field assignment expects =".into()));
                }
                let value = parser.expression(&memories).map_err(error)?;
                Command::DatabaseFieldAssign(name, field, value)
            }
            _ if memories.contains(&name) && parser.take(Token::Symbol("(")) => {
                let index = parser.expression(&memories).map_err(error)?;
                if !parser.take(Token::Symbol(")")) || !parser.take(Token::Symbol("=")) {
                    return Err(error("expected ) = after array index".into()));
                }
                let value = parser.expression(&memories).map_err(error)?;
                Command::ArrayAssign(name, index, value)
            }
            _ if memories.contains(&name) => parser.assignment(name, &memories).map_err(error)?,
            _ if let Some(operation) = host_operation(&name) => {
                if operation == Operation::Background {
                    if !parser.take(Token::Symbol("=")) {
                        return Err(error("Fundo requires = before its image path".into()));
                    }
                } else {
                    parser.take(Token::Symbol("="));
                }
                let arguments = if operation == Operation::ConfigureWindow {
                    parser.window_arguments(OperandContext::Output, &memories)
                } else {
                    parser.arguments(OperandContext::Output, &memories)
                }
                .map_err(error)?;
                validate_host_argument_count(operation, arguments.len()).map_err(error)?;
                Command::Host(operation, arguments)
            }
            _ if parser.take(Token::Symbol("("))
                && parser.take(Token::Symbol(")"))
                && parser.tokens.is_empty() =>
            {
                if routines
                    .insert(name.clone(), statements.len() + 1)
                    .is_some()
                {
                    return Err(error(format!("duplicate routine {name}")));
                }
                Command::RoutineStart
            }
            _ if !reserved(&name) && parser.tokens.is_empty() && destinations.contains(&name) => {
                declaring = false;
                if labels.insert(name.clone(), statements.len()).is_some() {
                    return Err(error(format!("duplicate label {name}")));
                }
                continue;
            }
            _ => {
                return Err(error(format!(
                    "unsupported command or unknown memory: {name}"
                )));
            }
        };
        if matches!(command, Command::RoutineStart) && !main_ended {
            return Err(error("routine definitions belong after FimPrograma".into()));
        }
        main_ended |= matches!(command, Command::EndProgram | Command::EndWindow);
        header &= matches!(
            command,
            Command::Declare(_) | Command::HostBlock(Operation::DatabaseDefine, _, _)
        );
        declaring &= matches!(command, Command::Declare(_));
        parser.end().map_err(error)?;
        statements.push(Statement { line, command });
    }
    if let Some(previous) = conditions.first() {
        return Err(Error::new(
            statements[*previous].line,
            "Se block is missing FimSe".into(),
        ));
    }
    for (index, label) in jumps {
        let destination = labels
            .get(&label)
            .ok_or_else(|| Error::new(statements[index].line, format!("unknown label {label}")))?;
        statements[index].command = Command::Jump(*destination);
    }
    for (index, name) in calls {
        let destination = routines
            .get(&name)
            .ok_or_else(|| Error::new(statements[index].line, format!("unknown routine {name}")))?;
        statements[index].command = Command::Call(*destination);
    }
    Ok(statements)
}

fn validate_host_argument_count(operation: Operation, count: usize) -> Result<(), String> {
    let valid = match operation {
        Operation::Background | Operation::StopSound | Operation::BoxAdd => count == 1,
        Operation::Draw | Operation::Play | Operation::PrintDraw => (1..=2).contains(&count),
        Operation::Component
        | Operation::BoxPosition
        | Operation::BoxTitle
        | Operation::PrintPosition => count == 2,
        Operation::RemoveButtons
        | Operation::BoxActivate
        | Operation::BoxNew
        | Operation::BoxDeactivate
        | Operation::PrintNewPage
        | Operation::PrintFinish => count == 0,
        Operation::PrintWrite => count >= 1,
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{operation:?} received an invalid number of arguments"
        ))
    }
}

fn validate_database_argument_count(operation: Operation, count: usize) -> Result<(), String> {
    let valid = match operation {
        Operation::DatabaseOpen => (1..=2).contains(&count),
        Operation::DatabaseClose | Operation::DatabaseAppend | Operation::DatabaseCompact => {
            count == 1
        }
        Operation::DatabaseRead | Operation::DatabaseDelete => count == 2,
        Operation::DatabaseOrder => count >= 2,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{operation:?} received an invalid number of arguments"
        ))
    }
}

fn reserved(name: &str) -> bool {
    const WORDS: &str = "
        MEMORIA POSICIONE ESCREVA LIMPAJANELA SE CASOCONTRARIO FIMSE VAPARA
        FIMPROGRAMA FIMJANELA SIM NAO E OU ENTRATEXTO
        MENSAGEM FUNDO FONTE JANELA COMPONENTE AGUARDE CRIA FIMCRIA DESENHE
        DESENHEOBJETOS POSICIONEOBJETO TRANSPARENTE SOM FIMSOM VIDEO TESTAMOUSE
        EXECUTE RETORNE UNIR SEPARE DATA HORA FORMATA PROCUREMATRIZ ENTRE
        GRAVAMEMORIA CARREGAMEMORIA APAGAMEMORIA REORGANIZAMEMORIA CARREGAPROGRAMA
        MENU FIMMENU BOTAO FIMBOTAO RGB IMP ALEATORIO
        BD FIMBD ABREBD FECHABD ADICIONAREGISTRO LEREGISTRO ORDEMBD PROCURE TOTALREGISTROS
        LIMPABD EXCLUIREGISTRO NOVAPAGINA
        RETORNO LINHAMOUSE COLUNAMOUSE CLIQUEMOUSE CORATUAL AZUL VERMELHO VERDE AMARELO
        AZULCLARO AZULESCURO VERMELHOESCURO VERDEESCURO AMARELOESCURO RETORNOBOX
        TOTALFONTES PASTAPROGRAMA PRETO BRANCO PASTAWINDOWS AREATRABALHO MENUPROGRAMAS
        EDTOTAL EDCOLUNA EDTEXTOCAMPO TESTABOTAO CLIQUEOBJETO CLIQUEBOTAO
        BOXPOSICAO BOXCONFIG BOXTITULO BOXATIVA BOXADD BOXNEW BOXDESATIVA
    ";
    WORDS.split_ascii_whitespace().any(|word| word == name)
}

fn builtin(name: &str) -> bool {
    matches!(
        name,
        "UNIR"
            | "SEPARE"
            | "FORMATA"
            | "ALINHA"
            | "RGB"
            | "ALEATORIO"
            | "ENTRE"
            | "DATA"
            | "HORA"
            | "TESTAMOUSE"
            | "TOTALREGISTROS"
            | "PROCURE"
    )
}

fn builtin_takes_arguments(name: &str) -> bool {
    matches!(
        name,
        "UNIR"
            | "SEPARE"
            | "FORMATA"
            | "ALINHA"
            | "RGB"
            | "ALEATORIO"
            | "ENTRE"
            | "TOTALREGISTROS"
            | "PROCURE"
    )
}

fn reserved_value(name: &str) -> bool {
    matches!(
        name,
        "LINHAMOUSE"
            | "COLUNAMOUSE"
            | "CLIQUEMOUSE"
            | "CORATUAL"
            | "RETORNOBOX"
            | "TOTALFONTES"
            | "PASTAPROGRAMA"
            | "PASTAWINDOWS"
            | "AREATRABALHO"
            | "MENUPROGRAMAS"
            | "EDTOTAL"
            | "EDCOLUNA"
            | "EDTEXTOCAMPO"
            | "TESTABOTAO"
            | "AZUL"
            | "VERMELHO"
            | "VERDE"
            | "AMARELO"
            | "AZULCLARO"
            | "AZULESCURO"
            | "VERMELHOESCURO"
            | "VERDEESCURO"
            | "AMARELOESCURO"
            | "PRETO"
            | "BRANCO"
    )
}

fn string_constant(name: &str) -> bool {
    matches!(
        name,
        "PADRAO"
            | "DIR"
            | "ESQ"
            | "PRIMEIRO"
            | "ULTIMO"
            | "PROXIMO"
            | "SEQUENCIAL"
            | "FRENTE"
            | "ATRAS"
            | "OPACO"
            | "NORMAL"
            | "OTIMIZADO"
            | "CORTRANSPARENTE"
            | "MODO"
            | "OBJETO"
            | "SOM"
            | "ESPECIAL"
    )
}

fn host_operation(name: &str) -> Option<Operation> {
    Some(match name {
        "FUNDO" => Operation::Background,
        "FONTE" => Operation::Font,
        "JANELA" => Operation::ConfigureWindow,
        "DESENHE" => Operation::Draw,
        "DESENHEOBJETOS" => Operation::DrawObjects,
        "POSICIONEOBJETO" => Operation::PositionObject,
        "TRANSPARENTE" => Operation::Transparency,
        "SOM" => Operation::Play,
        "FIMSOM" => Operation::StopSound,
        "VIDEO" => Operation::Video,
        "TESTAMOUSE" => Operation::MouseButtons,
        "COMPONENTE" => Operation::Component,
        "MENU" => Operation::Menu,
        "FIMMENU" => Operation::Menu,
        "BOTAO" => Operation::Buttons,
        "FIMBOTAO" => Operation::RemoveButtons,
        "IMP" => Operation::PrintWrite,
        "GRAVAMEMORIA" => Operation::ReservedMemory,
        "CARREGAMEMORIA" => Operation::ReservedMemory,
        "APAGAMEMORIA" => Operation::ReservedMemory,
        "REORGANIZAMEMORIA" => Operation::ReservedMemory,
        "DATA" => Operation::Date,
        "HORA" => Operation::Time,
        "PROCUREMATRIZ" => Operation::ReservedMemory,
        "ABREBD" => Operation::DatabaseOpen,
        "EXCLUIREGISTRO" => Operation::DatabaseDelete,
        "FECHABD" => Operation::DatabaseClose,
        "ADICIONAREGISTRO" => Operation::DatabaseAppend,
        "LEREGISTRO" => Operation::DatabaseRead,
        "ORDEMBD" => Operation::DatabaseOrder,
        "PROCURE" => Operation::DatabaseFind,
        "TOTALREGISTROS" => Operation::DatabaseCount,
        "LIMPABD" => Operation::DatabaseCompact,
        "BOXPOSICAO" => Operation::BoxPosition,
        "BOXCONFIG" => Operation::BoxConfig,
        "BOXTITULO" => Operation::BoxTitle,
        "BOXATIVA" => Operation::BoxActivate,
        "BOXADD" => Operation::BoxAdd,
        "BOXNEW" => Operation::BoxNew,
        "BOXDESATIVA" => Operation::BoxDeactivate,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OperandContext {
    Assignment,
    Argument,
    Output,
    Condition,
    Wait,
}

struct Parser<'tokens, 'source> {
    tokens: &'tokens [Token<'source>],
}

impl<'source> Parser<'_, 'source> {
    fn next(&mut self) -> Option<Token<'source>> {
        let (first, rest) = self.tokens.split_first()?;
        self.tokens = rest;
        Some(*first)
    }

    fn take(&mut self, token: Token<'_>) -> bool {
        if self.tokens.first() == Some(&token) {
            self.next();
            true
        } else {
            false
        }
    }

    fn keyword(&mut self, word: &str) -> bool {
        if matches!(self.tokens.first(), Some(Token::Name(name)) if name.eq_ignore_ascii_case(word))
        {
            self.next();
            true
        } else {
            false
        }
    }

    fn name(&mut self) -> Result<String, String> {
        match self.next() {
            Some(Token::Name(name)) => Ok(name.to_ascii_uppercase()),
            _ => Err("expected a command or identifier".into()),
        }
    }

    fn end(&self) -> Result<(), String> {
        match self.tokens.first() {
            None => Ok(()),
            Some(Token::Symbol("(")) => Err("array or routine syntax is unsupported".into()),
            _ => Err("unexpected arguments or unsupported syntax".into()),
        }
    }

    fn comma(&mut self) -> Result<(), String> {
        self.take(Token::Symbol(","))
            .then_some(())
            .ok_or_else(|| "expected an argument separator (,)".into())
    }

    fn operand(
        &mut self,
        context: OperandContext,
        memories: &HashSet<String>,
    ) -> Result<Operand, String> {
        let negative = self.take(Token::Symbol("-"));
        let mut decimal = None;
        let value = match self.next() {
            Some(Token::Digits(integer)) => {
                let number = match self.tokens {
                    [Token::Symbol(","), Token::Digits(fraction), rest @ ..]
                        if matches!(context, OperandContext::Assignment | OperandContext::Wait) =>
                    {
                        self.tokens = rest;
                        finite_number(&format!("{integer}.{fraction}"))?
                    }
                    _ => finite_number(integer)?,
                };
                Atom::Literal(Value::Number(if negative { -number } else { number }))
            }
            Some(Token::Text(text)) if !negative => {
                if context == OperandContext::Condition {
                    decimal = quoted_decimal(text);
                }
                Atom::Literal(Value::Text(text.into()))
            }
            Some(Token::Name(name)) if !negative => {
                let upper = name.to_ascii_uppercase();
                if builtin(&upper) {
                    if context != OperandContext::Assignment {
                        return Err("commands cannot be passed as arguments".into());
                    }
                    let arguments = self.builtin_arguments(&upper, context, memories)?;
                    Atom::Builtin(upper, arguments)
                } else if self.take(Token::Symbol("(")) {
                    let index = self.expression(memories)?;
                    if !self.take(Token::Symbol(")")) {
                        return Err("expected ) after array index".into());
                    }
                    return Ok(Operand {
                        value: Atom::IndexedMemory(upper, Box::new(index)),
                        decimal: None,
                        name: None,
                    });
                } else if self.take(Token::Symbol(".")) {
                    Atom::DatabaseField(upper, self.name()?)
                } else {
                    match upper.as_str() {
                        "SIM" => Atom::Literal(Value::Logic(true)),
                        "NAO" => Atom::Literal(Value::Logic(false)),
                        "RETORNO" => Atom::Memory(upper),
                        "CLIQUEOBJETO" | "CLIQUEBOTAO" => Atom::Literal(Value::Text(name.into())),
                        _ if reserved_value(&upper) => Atom::Reserved(upper),
                        _ if string_constant(&upper) => Atom::Literal(Value::Text(name.to_owned())),
                        _ if memories.contains(&upper) => Atom::Memory(upper),
                        _ if context == OperandContext::Output && !reserved(&upper) => {
                            Atom::Literal(Value::Text(name.into()))
                        }
                        _ => return Err(format!("unsupported operand or unknown memory: {name}")),
                    }
                }
            }
            _ => return Err("expected a value".into()),
        };
        Ok(Operand {
            value,
            decimal,
            name: None,
        })
    }

    fn builtin_arguments(
        &mut self,
        name: &str,
        context: OperandContext,
        memories: &HashSet<String>,
    ) -> Result<Vec<Operand>, String> {
        let parenthesized = self.take(Token::Symbol("("));
        let mut arguments = Vec::new();
        if !builtin_takes_arguments(name) {
            if parenthesized && !self.take(Token::Symbol(")")) {
                return Err(format!("{name} does not take arguments"));
            }
            return Ok(arguments);
        }
        if parenthesized || builtin_takes_arguments(name) {
            if !parenthesized || !self.take(Token::Symbol(")")) {
                let argument_context = if context == OperandContext::Condition {
                    context
                } else {
                    OperandContext::Output
                };
                arguments.push(self.operand(argument_context, memories)?);
                while self.take(Token::Symbol(",")) {
                    arguments.push(self.operand(argument_context, memories)?);
                }
                if parenthesized && !self.take(Token::Symbol(")")) {
                    return Err("expected ) after function arguments".into());
                }
            }
        }
        match name {
            "TOTALREGISTROS" if arguments.len() != 1 => {
                return Err("TotalRegistros expects one database name".into());
            }
            "PROCURE" if arguments.len() < 3 => {
                return Err("Procure expects a database, mode, and search value".into());
            }
            _ => {}
        }
        Ok(arguments)
    }

    fn arguments(
        &mut self,
        context: OperandContext,
        memories: &HashSet<String>,
    ) -> Result<Vec<Operand>, String> {
        self.argument_list(context, memories, false, false)
    }

    fn arguments_with_whitespace(
        &mut self,
        context: OperandContext,
        memories: &HashSet<String>,
    ) -> Result<Vec<Operand>, String> {
        self.argument_list(context, memories, false, true)
    }

    fn window_arguments(
        &mut self,
        context: OperandContext,
        memories: &HashSet<String>,
    ) -> Result<Vec<Operand>, String> {
        self.argument_list(context, memories, true, false)
    }

    fn argument_list(
        &mut self,
        context: OperandContext,
        memories: &HashSet<String>,
        paired_dimensions: bool,
        allow_whitespace_separator: bool,
    ) -> Result<Vec<Operand>, String> {
        let mut arguments = Vec::new();
        while !self.tokens.is_empty() {
            let name = match self.tokens {
                [Token::Name(name), Token::Symbol("="), ..] => Some(name.to_ascii_uppercase()),
                _ => None,
            };
            if name.is_some() {
                self.next();
                self.next();
            }
            let mut argument = self.operand(context, memories)?;
            argument.name = name;
            arguments.push(argument);
            if let Some(name) = arguments
                .last()
                .and_then(|argument| argument.name.as_deref())
                .filter(|name| {
                    paired_dimensions && matches!(*name, "RESOLUCAO" | "POSICAO" | "TAMANHO")
                })
                .map(str::to_owned)
            {
                self.comma()?;
                let mut second = self.operand(context, memories)?;
                second.name = Some(name);
                arguments.push(second);
            }
            if self.take(Token::Symbol(",")) {
                if self.tokens.is_empty() {
                    return Err("expected an argument after the separator".into());
                }
            } else if !allow_whitespace_separator && !self.tokens.is_empty() {
                return Err("expected an argument separator (,)".into());
            }
        }
        Ok(arguments)
    }

    fn assignment(&mut self, name: String, memories: &HashSet<String>) -> Result<Command, String> {
        match self.next() {
            Some(Token::Symbol("=")) if self.keyword("EntraTexto") => {
                self.comma()?;
                let prompt = self.operand(OperandContext::Argument, memories)?;
                Ok(Command::ReadText(name, prompt))
            }
            Some(Token::Symbol("=")) if self.keyword("ProcureMatriz") => {
                let array = self.name()?;
                if !memories.contains(&array) {
                    return Err(format!("unknown matrix {array}"));
                }
                self.comma()?;
                let target = self.operand(OperandContext::Argument, memories)?;
                Ok(Command::FindArray(name, array, target))
            }
            Some(Token::Symbol("=")) => Ok(Command::Assign(name, self.expression(memories)?)),
            Some(Token::Symbol("+=")) => Ok(Command::Update(
                name,
                Arithmetic::Add,
                self.expression(memories)?,
            )),
            Some(Token::Symbol("-=")) => Ok(Command::Update(
                name,
                Arithmetic::Subtract,
                self.expression(memories)?,
            )),
            Some(Token::Symbol(operator @ ("++" | "--"))) => {
                let expression = Expression {
                    first: Product {
                        first: Operand {
                            value: Atom::Literal(Value::Number(1.0)),
                            decimal: None,
                            name: None,
                        },
                        rest: Vec::new(),
                    },
                    rest: Vec::new(),
                };
                let operation = if operator == "++" {
                    Arithmetic::Add
                } else {
                    Arithmetic::Subtract
                };
                Ok(Command::Update(name, operation, expression))
            }
            _ => Err("expected an assignment or update to the memory".into()),
        }
    }

    fn product(&mut self, memories: &HashSet<String>) -> Result<Product, String> {
        let first = self.operand(OperandContext::Assignment, memories)?;
        let mut rest = Vec::new();
        loop {
            let operator = match self.tokens.first() {
                Some(Token::Symbol("*")) => Arithmetic::Multiply,
                Some(Token::Symbol("/")) => Arithmetic::Divide,
                Some(Token::Symbol("%")) => Arithmetic::Remainder,
                _ => break,
            };
            self.next();
            rest.push((
                operator,
                self.operand(OperandContext::Assignment, memories)?,
            ));
        }
        Ok(Product { first, rest })
    }

    fn expression(&mut self, memories: &HashSet<String>) -> Result<Expression, String> {
        let first = self.product(memories)?;
        let mut rest = Vec::new();
        loop {
            let operator = match self.tokens.first() {
                Some(Token::Symbol("+")) => Arithmetic::Add,
                Some(Token::Symbol("-")) => Arithmetic::Subtract,
                _ => break,
            };
            self.next();
            rest.push((operator, self.product(memories)?));
        }
        Ok(Expression { first, rest })
    }

    fn condition(&mut self, memories: &HashSet<String>) -> Result<Condition, String> {
        let mut condition = Vec::new();
        let mut conjunction = Vec::new();
        let mut used_and = false;
        let mut used_or = false;
        loop {
            let left = self.operand(OperandContext::Condition, memories)?;
            let operator = match self.next() {
                Some(Token::Symbol("=")) => Comparison::Equal,
                Some(Token::Symbol("<>")) => Comparison::NotEqual,
                Some(Token::Symbol("<")) => Comparison::Less,
                Some(Token::Symbol("<=")) => Comparison::LessOrEqual,
                Some(Token::Symbol(">")) => Comparison::Greater,
                Some(Token::Symbol(">=")) => Comparison::GreaterOrEqual,
                _ => return Err("expected a comparison operator in Se".into()),
            };
            let right = self.operand(OperandContext::Condition, memories)?;
            conjunction.push(Predicate {
                left,
                operator,
                right,
            });
            if self.keyword("E") {
                if used_or {
                    return Err("Se cannot mix E and OU on the same line".into());
                }
                used_and = true;
                continue;
            }
            condition.push(std::mem::take(&mut conjunction));
            if !self.keyword("OU") {
                return Ok(condition);
            }
            if used_and {
                return Err("Se cannot mix E and OU on the same line".into());
            }
            used_or = true;
        }
    }
}

fn finite_number(text: &str) -> Result<f64, String> {
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| "numeric literal is out of range".into())
}

fn quoted_decimal(text: &str) -> Option<f64> {
    let (integer, fraction) = text.split_once(',')?;
    let digits = integer.strip_prefix('-').unwrap_or(integer);
    if digits.is_empty()
        || fraction.is_empty()
        || !digits
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    finite_number(&format!("{integer}.{fraction}")).ok()
}
