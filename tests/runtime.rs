use std::collections::{BTreeMap, VecDeque};
use std::io;

use potoo_logic_basic_interpreter::{
    Host, HostMemory, HostValue, Operation, Records, Request, Snapshot, Window, run, run_with_host,
};

#[derive(Debug, Clone)]
struct RecordedRequest {
    operation: Operation,
    arguments: Vec<(Option<String>, HostValue)>,
    rows: Vec<Vec<(Option<String>, HostValue)>>,
    window_size: (usize, usize),
}

#[derive(Default)]
struct TestHost {
    responses: VecDeque<HostValue>,
    requests: Vec<RecordedRequest>,
    checkpoints: Vec<usize>,
    saved_memories: Option<(String, Snapshot)>,
    loaded_memories: Option<Snapshot>,
    loaded_program: Option<(String, String)>,
    database: Records,
    saved_database: Option<(String, String, Records)>,
    closed_database: Option<(String, String)>,
}

impl Host for TestHost {
    fn request(&mut self, request: Request<'_>) -> io::Result<Option<HostValue>> {
        self.requests.push(RecordedRequest {
            operation: request.operation,
            arguments: request
                .arguments
                .iter()
                .map(|argument| (argument.name.clone(), argument.value.clone()))
                .collect(),
            rows: request
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|argument| (argument.name.clone(), argument.value.clone()))
                        .collect()
                })
                .collect(),
            window_size: (request.window.rows(), request.window.columns()),
        });
        Ok(self.responses.pop_front())
    }

    fn checkpoint(&mut self, line: usize) -> io::Result<()> {
        self.checkpoints.push(line);
        Ok(())
    }

    fn save_memories(&mut self, path: &str, snapshot: &Snapshot) -> io::Result<()> {
        self.saved_memories = Some((path.to_owned(), snapshot.clone()));
        Ok(())
    }

    fn load_memories(&mut self, _path: &str) -> io::Result<Option<Snapshot>> {
        Ok(self.loaded_memories.clone())
    }

    fn load_program(&mut self, path: &str) -> io::Result<String> {
        self.loaded_program
            .as_ref()
            .filter(|(expected, _)| expected == path)
            .map(|(_, source)| source.clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "program not found"))
    }

    fn open_database(
        &mut self,
        _directory: &str,
        _file: &str,
        _fields: &[(String, usize)],
    ) -> io::Result<Records> {
        Ok(self.database.clone())
    }

    fn save_database(&mut self, directory: &str, file: &str, records: &Records) -> io::Result<()> {
        self.saved_database = Some((directory.into(), file.into(), records.clone()));
        Ok(())
    }

    fn close_database(&mut self, directory: &str, file: &str) -> io::Result<()> {
        self.closed_database = Some((directory.into(), file.into()));
        Ok(())
    }
}

fn cell_text(window: &Window, row: usize, columns: usize) -> String {
    (0..columns)
        .map(|column| window.cell(row, column).unwrap())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn callback_input_receives_prompt_and_program_writes_to_text_grid() {
    let mut received_prompt = String::new();
    let window = run(
        "Memoria Nome\nNome = EntraTexto, \"Qual é seu nome?\"\nEscreva \"Olá, \", Nome\nFimPrograma",
        |prompt, _| {
            received_prompt = prompt.to_owned();
            Ok("Ada".to_owned())
        },
    )
    .unwrap()
    .unwrap();

    assert_eq!(received_prompt, "Qual é seu nome?");
    assert_eq!(cell_text(&window, 0, 79), "Olá, Ada");
}

#[test]
fn arithmetic_conditionals_and_semicolon_commands_keep_their_order() {
    let window = run(
        "Memoria Valor, Resultado\nValor = 2,5; Resultado = Valor + 1\nSe Resultado = \"3,5\"\nEscreva \"certo\"\nCasoContrario\nEscreva \"errado\"\nFimSe\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "certo");
    assert!(cell_text(&window, 1, 79).is_empty());
}

#[test]
fn array_lookup_returns_zero_based_index_and_missing_value_returns_minus_one() {
    let window = run(
        "Memoria Itens(3), Encontrado, Ausente\nItens(0) = \"maçã\"\nItens(1) = \"pera\"\nItens(2) = \"uva\"\nEncontrado = ProcureMatriz Itens, \"pera\"\nAusente = ProcureMatriz Itens, \"banana\"\nEscreva Encontrado, \":\", Ausente\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "1:-1");
}

#[test]
fn string_builtins_join_slice_align_and_format_values() {
    let window = run(
        "Memoria Junto, Parte, Alinhado, Formatado\nJunto = Unir(\"Logic\", \" Basic\")\nParte = Separe(Junto, 7, 5)\nAlinhado = Alinha(\"oi\", 4, DIR)\nFormatado = Formata(7, \"000\")\nEscreva Parte, \"|\", Alinhado, \"|\", Formatado\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "Basic|  oi|007");
}

#[test]
fn named_routine_returns_to_the_call_site() {
    let window = run(
        "Memoria Contagem\nContagem = 0\nExecute Incrementa()\nEscreva Contagem\nFimPrograma\nIncrementa()\nContagem++\nRetorne",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "1");
}

#[test]
fn host_receives_window_configuration_and_menu_rows_as_typed_operations() {
    let mut host = TestHost::default();
    let window = run_with_host(
        "Janela RESOLUCAO = 3, 10\nMenu \"Opções\"\n\"Iniciar\"\n\"Sair\"\nFimMenu\nFimPrograma",
        &mut host,
    )
    .unwrap()
    .unwrap();

    assert_eq!((window.rows(), window.columns()), (3, 10));
    assert_eq!(host.requests.len(), 2);
    assert_eq!(host.requests[0].operation, Operation::ConfigureWindow);
    assert_eq!(host.requests[0].window_size, (3, 10));
    assert_eq!(host.requests[1].operation, Operation::Menu);
    assert_eq!(
        host.requests[1].arguments[0].1,
        HostValue::Text("Opções".into())
    );
    assert_eq!(
        host.requests[1].rows,
        vec![
            vec![(None, HostValue::Text("Iniciar".into()))],
            vec![(None, HostValue::Text("Sair".into()))]
        ]
    );
}

#[test]
fn memory_snapshots_are_saved_and_loaded_through_the_host() {
    let mut host = TestHost::default();
    host.loaded_memories = Some(BTreeMap::from([(
        "PONTOS".to_owned(),
        HostMemory::Scalar(HostValue::Number(9.0)),
    )]));

    let window = run_with_host(
        "Memoria Pontos\nPontos = 2\nGravaMemoria \"progresso.mem\"\nPontos = 0\nCarregaMemoria \"progresso.mem\"\nEscreva Pontos\nFimPrograma",
        &mut host,
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "9");
    let (path, snapshot) = host.saved_memories.unwrap();
    assert_eq!(path, "progresso.mem");
    assert_eq!(
        snapshot.get("PONTOS"),
        Some(&HostMemory::Scalar(HostValue::Number(2.0)))
    );
}

#[test]
fn loaded_program_runs_in_the_existing_window() {
    let mut host = TestHost {
        loaded_program: Some((
            "parte.lbc".into(),
            "Memoria Parte\nParte = 7\nEscreva Parte".into(),
        )),
        ..TestHost::default()
    };

    let window = run_with_host(
        "CarregaPrograma \"parte.lbc\"\nEscreva \"fim\"\nFimPrograma",
        &mut host,
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "7");
    assert_eq!(cell_text(&window, 1, 79), "fim");
}

#[test]
fn database_records_can_be_appended_sorted_read_and_persisted_by_the_host() {
    let mut host = TestHost::default();
    let window = run_with_host(
        "BD Alunos, \"dados\"\nNome, 8\nNota, 3\nFimBD\nAbreBD Alunos, \"turma\"\nAlunos.Nome = \"Bia\"\nAlunos.Nota = 9\nAdicionaRegistro Alunos\nAlunos.Nome = \"Ada\"\nAlunos.Nota = 10\nAdicionaRegistro Alunos\nOrdemBD Alunos, Nome\nLeRegistro Alunos, 0\nEscreva Alunos.Nome\nFechaBD Alunos\nFimPrograma",
        &mut host,
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "Ada");
    assert_eq!(
        host.closed_database.as_ref(),
        Some(&("dados".into(), "turma.LBA".into()))
    );
    let (directory, file, records) = host.saved_database.unwrap();
    assert_eq!((directory.as_str(), file.as_str()), ("dados", "turma.LBA"));
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].as_ref().unwrap().get("Nome"),
        Some(&HostValue::Text("Bia".into()))
    );
    assert_eq!(
        records[1].as_ref().unwrap().get("Nome"),
        Some(&HostValue::Text("Ada".into()))
    );
}

#[test]
fn checkpoint_interruption_is_reported_as_a_stopped_execution() {
    struct StopHost;
    impl Host for StopHost {
        fn checkpoint(&mut self, line: usize) -> io::Result<()> {
            assert_eq!(line, 2);
            Err(io::ErrorKind::Interrupted.into())
        }
    }

    let error = run_with_host("\nEscreva \"parar\"", &mut StopHost).unwrap_err();

    assert_eq!(error.line(), 2);
    assert!(error.was_stopped());
}

#[test]
fn end_window_closes_the_window_without_returning_a_grid() {
    let result = run("FimJanela", |_, _| {
        unreachable!("program does not request input")
    })
    .unwrap();

    assert!(result.is_none());
}
