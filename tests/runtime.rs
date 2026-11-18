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
    window_cursor: (usize, usize),
}

#[derive(Default)]
struct TestHost {
    responses: VecDeque<HostValue>,
    requests: Vec<RecordedRequest>,
    checkpoints: Vec<usize>,
    saved_memories: Option<(String, Snapshot)>,
    loaded_memories: Option<Snapshot>,
    loaded_memory_paths: Vec<String>,
    loaded_program: Option<(String, String)>,
    loaded_program_paths: Vec<String>,
    database: Records,
    opened_databases: Vec<(String, String, Vec<(String, usize)>)>,
    saved_database: Option<(String, String, Records)>,
    closed_database: Option<(String, String)>,
    database_saves: Vec<Records>,
    database_save_failures: usize,
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
            window_cursor: request.window.cursor(),
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

    fn load_memories(&mut self, path: &str) -> io::Result<Option<Snapshot>> {
        self.loaded_memory_paths.push(path.to_owned());
        Ok(self.loaded_memories.clone())
    }

    fn load_program(&mut self, path: &str) -> io::Result<String> {
        self.loaded_program_paths.push(path.to_owned());
        self.loaded_program
            .as_ref()
            .filter(|(expected, _)| expected == path)
            .map(|(_, source)| source.clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "program not found"))
    }

    fn open_database(
        &mut self,
        directory: &str,
        file: &str,
        fields: &[(String, usize)],
    ) -> io::Result<Records> {
        self.opened_databases
            .push((directory.to_owned(), file.to_owned(), fields.to_vec()));
        Ok(self.database.clone())
    }

    fn save_database(&mut self, directory: &str, file: &str, records: &Records) -> io::Result<()> {
        self.database_saves.push(records.clone());
        if self.database_save_failures > 0 {
            self.database_save_failures -= 1;
            return Err(io::Error::other("database save failed"));
        }
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
fn limpa_janela_clears_text_before_subsequent_writes() {
    let window = run(
        "Posicione 2, 3; Escreva \"antigo\"; LimpaJanela; Posicione 2, 3; Escreva \"novo\"; FimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 2, 79), "   novo");
    assert_eq!(
        (3..7)
            .map(|c| window.cell(2, c).unwrap())
            .collect::<String>(),
        "novo"
    );
}

#[test]
fn documented_text_grid_coordinates_and_argument_joining_are_preserved() {
    let window = run(
        "Posicione 11, 33\nEscreva \"Alô \", \"Mundo!\"\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(window.rows(), 27);
    assert_eq!(window.columns(), 79);
    assert_eq!(window.cell(11, 33), Some('A'));
    let positioned = (33..43)
        .map(|column| window.cell(11, column).unwrap())
        .collect::<String>();
    assert_eq!(positioned, "Alô Mundo!");
}

#[test]
fn text_grid_advances_after_each_write_and_clips_at_the_row_boundary() {
    let mut host = TestHost::default();
    let window = run_with_host(
        "Escreva \"primeiro\"\nEscreva \"segundo\"\nPosicione 2, 77\nEscreva \"ABCD\"\nFundo = \"fundo.bmp\"\nFimPrograma",
        &mut host,
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "primeiro");
    assert_eq!(cell_text(&window, 1, 79), "segundo");
    assert_eq!(window.cell(2, 77), Some('A'));
    assert_eq!(window.cell(2, 78), Some('B'));
    assert_eq!(window.cell(3, 0), Some(' '));
    let background = host
        .requests
        .iter()
        .find(|request| request.operation == Operation::Background)
        .unwrap();
    assert_eq!(background.window_cursor, (3, 0));
}

#[test]
fn clearing_the_grid_preserves_the_write_cursor() {
    let mut host = TestHost::default();
    let window = run_with_host(
        "Posicione 4, 5\nEscreva \"apagado\"\nLimpaJanela\nFundo = \"fundo.bmp\"\nFimPrograma",
        &mut host,
    )
    .unwrap()
    .unwrap();

    assert!((0..window.rows()).all(|row| cell_text(&window, row, window.columns()).is_empty()));
    let background = host
        .requests
        .iter()
        .find(|request| request.operation == Operation::Background)
        .unwrap();
    assert_eq!(background.window_cursor, (5, 0));
}

#[test]
fn message_joins_multiple_values_before_sending_the_host_request() {
    let mut host = TestHost::default();
    let result = run_with_host(
        "Memoria Nome\nNome = \"Ada\"\nMensagem \"Olá, \", Nome\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(host.requests.len(), 1);
    assert_eq!(host.requests[0].operation, Operation::Message);
    assert_eq!(
        host.requests[0].arguments,
        vec![(None, HostValue::Text("Olá, Ada".into()))]
    );
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
fn array_indices_reject_negative_fractional_and_out_of_range_positions() {
    let cases = [
        (
            "Memoria Itens(2)\nItens(2) = \"fora\"",
            "array index 2 is out of range",
        ),
        (
            "Memoria Itens(2)\nEscreva Itens(2)",
            "array index 2 is out of range",
        ),
        (
            "Memoria Itens(2)\nEscreva Itens(-1)",
            "array index must be a nonnegative integer",
        ),
        (
            "Memoria Itens(2)\nEscreva Itens(1,5)",
            "array index must be a nonnegative integer",
        ),
    ];

    for (source, expected_error) in cases {
        let result = run(source, |_, _| {
            unreachable!("program does not request input")
        });
        let error = result.unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "unexpected error for {source:?}: {error}"
        );
    }
}

#[test]
fn undeclared_array_elements_have_empty_text_content() {
    let window = run(
        "Memoria Nomes(2)\nNomes(0) = \"Ana\"\nEscreva \"<\", Nomes(1), \">\"\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "<>");
}

#[test]
fn separe_counts_accented_text_by_character_not_utf8_byte() {
    let window = run(
        "Memoria Letra\nLetra = Separe \"João\", 3, 1\nEscreva Letra\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "ã");
}

#[test]
fn string_builtins_join_slice_align_and_format_values() {
    let window = run(
        "Memoria Junto, Parte, Resto, Direita, Esquerda, Formatado\nJunto = Unir \"Logic\", \" Basic\"\nParte = Separe Junto, 7, 5\nResto = Separe Junto, 7\nDireita = Alinha \"oi\", 4, DIR\nEsquerda = Alinha \"oi\", 4, ESQ\nFormatado = Formata 7, \"000\"\nEscreva Parte, \"|\", Resto, \"|\", Direita, \"|\", Esquerda, \"|\", Formatado\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "Basic|Basic|  oi|oi  |007");
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
fn assigning_a_memory_copies_its_value_for_a_named_routine() {
    let window = run(
        "Memoria Original, Copia\nOriginal = \"antes\"\nCopia = Original\nOriginal = \"depois\"\nExecute MostraCopia()\nFimPrograma\nMostraCopia()\nEscreva Copia\nRetorne",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "antes");
}

#[test]
fn fim_janela_can_separate_the_main_program_from_routine_definitions() {
    let mut host = TestHost::default();
    let result = run_with_host("Execute Fechar()\nFimJanela\nFechar()\nRetorne", &mut host);

    assert!(result.is_ok(), "{result:?}");
    assert!(result.unwrap().is_none());
}

#[test]
fn aguarde_memory_operands_are_key_destinations_not_existing_values() {
    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Text("x".into()), HostValue::Text("y".into())]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Tecla, Outra\nTecla = \"CliqueBotao\"\nOutra = 5\nAguarde Tecla\nAguarde Outra\nEscreva Tecla, Outra\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        host.requests
            .iter()
            .map(|request| request.operation)
            .collect::<Vec<_>>(),
        vec![Operation::WaitKey, Operation::WaitKey]
    );
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "xy");
}

#[test]
fn commands_must_be_assigned_to_memory_before_their_values_are_used() {
    let mut host = TestHost::default();
    let result = run_with_host("Escreva Data", &mut host);

    assert!(result.is_err());
    assert!(host.requests.is_empty(), "nested Data reached the host");

    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Text("hoje".into())]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Hoje\nHoje = Data\nEscreva Hoje\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(host.requests[0].operation, Operation::Date);
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "hoje");
}

#[test]
fn host_receives_window_configuration_and_menu_rows_as_typed_operations() {
    let mut host = TestHost {
        responses: VecDeque::from([
            HostValue::Text(String::new()),
            HostValue::Text("Iniciar".into()),
        ]),
        ..TestHost::default()
    };
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
fn host_receives_the_current_text_cursor_for_positioned_drawing_and_menus() {
    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Text("Abrir".into())]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Posicione 3, 4\nMenu \"Ações\"\n\"Abrir\"\nFimMenu\nPosicione 7, 8\nDesenhe \"personagem.png\"\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(host.requests[0].operation, Operation::Menu);
    assert_eq!(host.requests[0].window_cursor, (3, 4));
    assert_eq!(host.requests[1].operation, Operation::Draw);
    assert_eq!(host.requests[1].window_cursor, (7, 8));
}

#[test]
fn scalar_font_sizes_and_window_dimension_pairs_keep_their_argument_shapes() {
    let mut host = TestHost::default();
    let result = run_with_host(
        "Fonte Nome = \"Arial\", Tamanho = 38, Negrito = SIM, Italico = NAO\nBoxConfig Nome = \"Arial\", Tamanho = 10\nImp Fonte Nome = \"Arial\", Tamanho = 38, Negrito = SIM, Italico = SIM\nJanela Resolucao = 3, 10, Posicao = 0, 0, Tamanho = 12, 40\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(host.requests.len(), 4);
    assert_eq!(host.requests[0].operation, Operation::Font);
    assert_eq!(
        host.requests[0].arguments,
        vec![
            (Some("NOME".into()), HostValue::Text("Arial".into())),
            (Some("TAMANHO".into()), HostValue::Number(38.0)),
            (Some("NEGRITO".into()), HostValue::Logic(true)),
            (Some("ITALICO".into()), HostValue::Logic(false)),
        ]
    );
    assert_eq!(host.requests[1].operation, Operation::BoxConfig);
    assert_eq!(
        host.requests[1].arguments,
        vec![
            (Some("NOME".into()), HostValue::Text("Arial".into())),
            (Some("TAMANHO".into()), HostValue::Number(10.0)),
        ]
    );
    assert_eq!(host.requests[2].operation, Operation::PrintFont);
    assert_eq!(
        host.requests[2].arguments[1],
        (Some("TAMANHO".into()), HostValue::Number(38.0))
    );
    assert_eq!(host.requests[3].operation, Operation::ConfigureWindow);
    assert_eq!(
        host.requests[3].arguments[4..],
        [
            (Some("TAMANHO".into()), HostValue::Number(12.0)),
            (Some("TAMANHO".into()), HostValue::Number(40.0)),
        ]
    );
    assert_eq!(
        host.requests[3].window_size,
        (3, 10),
        "Janela Resolucao must resize the text grid"
    );
}

#[test]
fn ordinary_and_special_buttons_require_their_documented_argument_counts() {
    let mut host = TestHost::default();
    let invalid = run_with_host("Botao\n\"OK\", 10, 10, 1\nFimBotao\nFimPrograma", &mut host);
    assert!(
        invalid.is_err(),
        "ordinary button without width was accepted"
    );

    let mut host = TestHost::default();
    let result = run_with_host(
        "Botao\n\"OK\", 10, 10, 1, 10\nFimBotao\nBotao Remove\nBotao Especial\n\"Sim\", 10, 10, 1\nFimBotao\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(host.requests.len(), 3);
    assert_eq!(host.requests[0].operation, Operation::Buttons);
    assert_eq!(host.requests[0].rows[0].len(), 5);
    assert_eq!(host.requests[1].operation, Operation::RemoveButtons);
    assert_eq!(host.requests[2].operation, Operation::Buttons);
    assert_eq!(host.requests[2].rows[0].len(), 4);
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
    assert_eq!(host.loaded_memory_paths, vec!["progresso.mem"]);
}

#[test]
fn memory_snapshot_load_overwrites_matches_and_preserves_other_named_values() {
    let mut host = TestHost {
        loaded_memories: Some(BTreeMap::from([
            (
                "VALOR".to_owned(),
                HostMemory::Scalar(HostValue::Number(9.0)),
            ),
            (
                "ITENS".to_owned(),
                HostMemory::Array(vec![
                    Some(HostValue::Text("salvo 0".into())),
                    Some(HostValue::Text("salvo 1".into())),
                ]),
            ),
            (
                "EXTRA".to_owned(),
                HostMemory::Scalar(HostValue::Text("presente".into())),
            ),
        ])),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Valor, Preservar, Itens(2)\nValor = 1\nPreservar = 2\nItens(0) = \"local\"\nCarregaMemoria \"snapshot.mem\"\nGravaMemoria \"snapshot.mem\"\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    let snapshot = host.saved_memories.unwrap().1;
    assert_eq!(
        snapshot.get("VALOR"),
        Some(&HostMemory::Scalar(HostValue::Number(9.0)))
    );
    assert_eq!(
        snapshot.get("PRESERVAR"),
        Some(&HostMemory::Scalar(HostValue::Number(2.0)))
    );
    assert_eq!(
        snapshot.get("ITENS"),
        Some(&HostMemory::Array(vec![
            Some(HostValue::Text("salvo 0".into())),
            Some(HostValue::Text("salvo 1".into())),
        ]))
    );
    assert_eq!(
        snapshot.get("EXTRA"),
        Some(&HostMemory::Scalar(HostValue::Text("presente".into())))
    );
    assert!(!snapshot.contains_key("RETORNO"));
}

#[test]
fn unassigned_snapshot_array_cells_read_as_empty_text() {
    let mut host = TestHost {
        loaded_memories: Some(BTreeMap::from([(
            "ITENS".to_owned(),
            HostMemory::Array(vec![None, Some(HostValue::Text("Ana".into()))]),
        )])),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Itens(2), Conteudo\nCarregaMemoria \"snapshot.mem\"\nConteudo = Itens(0)\nEscreva \"<\", Conteudo, \">\"\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "<>");
}

#[test]
fn procure_matriz_matches_unassigned_snapshot_cells_as_empty_text() {
    let mut host = TestHost {
        loaded_memories: Some(BTreeMap::from([(
            "ITENS".to_owned(),
            HostMemory::Array(vec![None, Some(HostValue::Text("Ana".into()))]),
        )])),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Itens(2), Posicao\nCarregaMemoria \"snapshot.mem\"\nPosicao = ProcureMatriz Itens, \"\"\nEscreva Posicao\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "0");
}

#[test]
fn missing_memory_snapshot_leaves_existing_values_unchanged() {
    let mut host = TestHost::default();
    let result = run_with_host(
        "Memoria Valor\nValor = 7\nCarregaMemoria \"ausente.mem\"\nGravaMemoria \"ausente.mem\"\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        host.saved_memories.unwrap().1.get("VALOR"),
        Some(&HostMemory::Scalar(HostValue::Number(7.0)))
    );
}

#[test]
fn reorganize_memories_has_no_visible_effect_on_named_arrays_or_snapshots() {
    let mut baseline_host = TestHost::default();
    let baseline = run_with_host(
        "Memoria Removida(2), Nomes(2)\nNomes(0) = \"Ana\"\nNomes(1) = \"Bia\"\nApagaMemoria Removida\nGravaMemoria \"memorias.mem\"\nFimPrograma",
        &mut baseline_host,
    );
    assert!(baseline.is_ok(), "{baseline:?}");
    let baseline = baseline_host.saved_memories.unwrap().1;

    let mut host = TestHost::default();
    let result = run_with_host(
        "Memoria Removida(2), Nomes(2)\nNomes(0) = \"Ana\"\nNomes(1) = \"Bia\"\nApagaMemoria Removida\nReorganizaMemoria\nGravaMemoria \"memorias.mem\"\nFimPrograma",
        &mut host,
    );
    assert!(result.is_ok(), "{result:?}");
    let snapshot = host.saved_memories.unwrap().1;
    assert_eq!(snapshot, baseline);
    assert!(!snapshot.contains_key("REMOVIDA"));
    assert_eq!(
        snapshot.get("NOMES"),
        Some(&HostMemory::Array(vec![
            Some(HostValue::Text("Ana".into())),
            Some(HostValue::Text("Bia".into())),
        ]))
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
    assert_eq!(host.loaded_program_paths, vec!["parte.lbc"]);
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
    assert_eq!(
        host.opened_databases,
        vec![(
            "dados".into(),
            "turma.LBA".into(),
            vec![("Nome".into(), 8), ("Nota".into(), 3)]
        )]
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
fn abrebd_accepts_the_documented_single_file_argument_for_one_declared_database() {
    let mut host = TestHost::default();
    let result = run_with_host(
        "BD Items \"data\"\n\"Name\", 5\nFimBD\nAbreBD \"october\"\nFechaBD Items\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        host.opened_databases,
        vec![(
            "data".into(),
            "october.LBA".into(),
            vec![("Name".into(), 5)]
        )]
    );
}

#[test]
fn abrebd_does_not_guess_a_schema_for_an_unmatched_file_with_multiple_databases() {
    let mut host = TestHost::default();
    let result = run_with_host(
        "BD Customers \"data\"\n\"Name\", 5\nFimBD\nBD Orders \"data\"\n\"Number\", 5\nFimBD\nAbreBD \"october\"",
        &mut host,
    );

    assert!(result.is_err());
    assert!(host.opened_databases.is_empty());
}

#[test]
fn failed_database_append_is_not_persisted_during_cleanup() {
    let mut host = TestHost {
        database_save_failures: 1,
        ..TestHost::default()
    };
    let result = run_with_host(
        "BD Items, \"data\"\n\"Name\", 5\nFimBD\nAbreBD Items, \"items\"\nItems.Name = \"Ada\"\nAdicionaRegistro Items\nFimPrograma",
        &mut host,
    );

    assert!(result.is_err());
    assert_eq!(host.database_saves.len(), 2);
    assert_eq!(host.database_saves[0].len(), 1);
    assert!(host.database_saves[0][0].is_some());
    assert!(host.database_saves[1].is_empty());
    assert_eq!(host.saved_database.unwrap().2, Vec::new());
}

#[test]
fn failed_database_delete_and_compaction_leave_records_unchanged() {
    let record = BTreeMap::from([("Name".to_owned(), HostValue::Text("Ada".into()))]);
    let mut delete_host = TestHost {
        database: vec![Some(record.clone())],
        database_save_failures: 1,
        ..TestHost::default()
    };
    let result = run_with_host(
        "BD Items, \"data\"\n\"Name\", 5\nFimBD\nAbreBD Items, \"items\"\nExcluiRegistro Items, 0\nFimPrograma",
        &mut delete_host,
    );
    assert!(result.is_err());
    assert_eq!(
        delete_host.database_saves,
        vec![vec![None], vec![Some(record.clone())]]
    );
    assert_eq!(
        delete_host.saved_database.unwrap().2,
        vec![Some(record.clone())]
    );

    let mut compact_host = TestHost {
        database: vec![None, Some(record.clone())],
        database_save_failures: 1,
        ..TestHost::default()
    };
    let result = run_with_host(
        "BD Items, \"data\"\n\"Name\", 5\nFimBD\nAbreBD Items, \"items\"\nLimpaBD Items\nFimPrograma",
        &mut compact_host,
    );
    assert!(result.is_err());
    assert_eq!(
        compact_host.database_saves,
        vec![vec![Some(record.clone())], vec![None, Some(record.clone())]]
    );
    assert_eq!(
        compact_host.saved_database.unwrap().2,
        vec![None, Some(record)]
    );
}

#[test]
fn database_order_is_total_for_numeric_and_text_values_in_one_field() {
    let mut host = TestHost::default();
    let result = run_with_host(
        "BD Items, \"data\"\n\"Key\", 10\nFimBD\nAbreBD Items, \"items\"\nItems.Key = \"11\"\nAdicionaRegistro Items\nItems.Key = 2\nAdicionaRegistro Items\nItems.Key = 10\nAdicionaRegistro Items\nOrdemBD Items, Key\nLeRegistro Items, 0\nEscreva Items.Key\nLeRegistro Items, 1\nEscreva Items.Key\nLeRegistro Items, 2\nEscreva Items.Key\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    let window = result.unwrap().unwrap();
    assert_eq!(cell_text(&window, 0, 79), "2");
    assert_eq!(cell_text(&window, 1, 79), "10");
    assert_eq!(cell_text(&window, 2, 79), "11");
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

#[test]
fn argument_lists_require_the_documented_comma_separator() {
    let mut host = TestHost::default();
    let result = run_with_host("Mensagem \"um\" \"dois\"", &mut host);

    assert!(result.is_err());
    assert!(host.requests.is_empty());
}

#[test]
fn fundo_requires_the_documented_equals_sign() {
    let mut host = TestHost::default();
    let result = run_with_host("Fundo \"fundo.bmp\"", &mut host);

    assert!(result.is_err());
    assert!(host.requests.is_empty());
}

#[test]
fn special_button_types_are_limited_to_the_documented_range() {
    let mut host = TestHost::default();
    let result = run_with_host(
        "Botao Especial\n\"Meu botão\", 1, 2, 10\nFimBotao",
        &mut host,
    );

    assert!(result.is_err());
    assert!(host.requests.is_empty());
}

#[test]
fn text_and_mouse_host_results_keep_their_documented_shapes() {
    let mut invalid_input = TestHost {
        responses: VecDeque::from([HostValue::Number(7.0)]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Nome\nNome = EntraTexto, \"Nome?\"\nFimPrograma",
        &mut invalid_input,
    );
    assert!(result.is_err());

    let mut invalid_mouse = TestHost {
        responses: VecDeque::from([HostValue::Text("1".into())]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Botao\nBotao = TestaMouse\nFimPrograma",
        &mut invalid_mouse,
    );
    assert!(result.is_err());
}

#[test]
fn recursive_routines_and_labels_follow_their_call_and_jump_targets() {
    let window = run(
        "Memoria N\nN = 3\nExecute Contagem()\nFimPrograma\nContagem()\nSe N > 0\nEscreva N\nN--\nExecute Contagem()\nFimSe\nRetorne",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(cell_text(&window, 0, 79), "3");
    assert_eq!(cell_text(&window, 1, 79), "2");
    assert_eq!(cell_text(&window, 2, 79), "1");

    let window = run(
        "Memoria N\nN = 0\nINICIO\nN++\nSe N < 3\nVaPara INICIO\nFimSe\nEscreva N\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(cell_text(&window, 0, 79), "3");
}

#[test]
fn recursive_routines_use_an_explicit_return_stack_beyond_shallow_depths() {
    let result = run(
        "Memoria N\nN = 1024\nExecute Contagem()\nFimPrograma\nContagem()\nSe N > 0\nN--\nExecute Contagem()\nFimSe\nRetorne",
        |_, _| unreachable!("program does not request input"),
    );

    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn documented_arithmetic_comparison_and_logical_operators_are_distinct() {
    let window = run(
        "Memoria X, Soma, Produto, Divisao, Resto, Atualizado\nX = 8\nSoma = 2 + 3 * 4\nProduto = 5 * 3\nDivisao = 8 / 2\nResto = 10 % 4\nAtualizado = 5\nAtualizado += 2\nAtualizado -= 1\nAtualizado++\nAtualizado--\nSe Soma = 14 E Produto >= 15\nEscreva \"A\"\nFimSe\nSe Divisao <> 3 OU Resto = 2\nEscreva \"B\"\nFimSe\nSe X < 9\nEscreva \"C\"\nFimSe\nSe X <= 8\nEscreva \"D\"\nFimSe\nSe X > 7\nEscreva \"E\"\nFimSe\nSe X >= 8\nEscreva \"F\"\nFimSe\nEscreva Atualizado\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "A");
    assert_eq!(cell_text(&window, 1, 79), "B");
    assert_eq!(cell_text(&window, 2, 79), "C");
    assert_eq!(cell_text(&window, 3, 79), "D");
    assert_eq!(cell_text(&window, 4, 79), "E");
    assert_eq!(cell_text(&window, 5, 79), "F");
    assert_eq!(cell_text(&window, 6, 79), "6");
}

#[test]
fn text_and_logical_values_use_their_own_comparison_semantics() {
    let window = run(
        "Se \"João\" = \"João\" E Sim = Sim\nEscreva \"true\"\nCasoContrario\nEscreva \"false\"\nFimSe\nSe Sim <> Nao\nEscreva \"|logical\"\nFimSe\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "true");
    assert_eq!(cell_text(&window, 1, 79), "|logical");
}

#[test]
fn documented_host_commands_keep_their_operation_and_argument_shapes() {
    let mut responses = vec![
        HostValue::Object("AVIAO".into()),
        HostValue::Sound("trilha".into()),
    ];
    responses.extend(vec![HostValue::Text(String::new()); 10]);
    responses.push(HostValue::Text("Um".into()));
    let mut host = TestHost {
        responses: responses.into(),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Aviao, Musica\nCria Objeto\nAviao = \"aviao.gif\"\nFimCria\nCria Som\nMusica = \"trilha.wav\"\nFimCria\nFundo = \"fundo.bmp\"\nFonte Nome=\"Arial\", Tamanho=12\nJanela Resolucao=25, 80, Posicao=0, 0, Tamanho=33, 100\nTransparente Modo=Otimizado, CorTransparente=\"Branco\"\nPosicione 2, 3\nDesenhe Aviao, Frente\nDesenheObjetos\nPosicioneObjeto Aviao, 4, 5\nSom Musica, 0\nFimSom 0\nVideo 5, 5, 25, 80, 0, \"Visivel\", \"SemControle\", \"TamanhoPadrao\", \"TelaNormal\"\nMenu \"Escolha\"\n\"Um\"\nFimMenu\nBotao\n\"Sim\", 1, 2, 3, 10\nFimBotao\nBotao Remove\nBoxPosicao 1, 2\nBoxConfig Nome=\"Arial\", Tamanho=10, CorFundo=\"Preto\", CorLetra=\"Branco\"\nBoxTitulo \"Nome\", 10\nBoxAtiva\nBoxAdd \"Ada\"\nBoxNew\nBoxDesativa\nImp Posicione 4, 5\nImp Escreva \"texto\", \" final\"\nImp Desenhe \"imagem.gif\"\nImp Fonte Nome=\"Arial\", Tamanho=12\nImp NovaPagina\nImp Fim\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        host.requests
            .iter()
            .map(|request| request.operation)
            .collect::<Vec<_>>(),
        vec![
            Operation::CreateObject,
            Operation::CreateSound,
            Operation::Background,
            Operation::Font,
            Operation::ConfigureWindow,
            Operation::Transparency,
            Operation::Draw,
            Operation::DrawObjects,
            Operation::PositionObject,
            Operation::Play,
            Operation::StopSound,
            Operation::Video,
            Operation::Menu,
            Operation::Buttons,
            Operation::RemoveButtons,
            Operation::BoxPosition,
            Operation::BoxConfig,
            Operation::BoxTitle,
            Operation::BoxActivate,
            Operation::BoxAdd,
            Operation::BoxNew,
            Operation::BoxDeactivate,
            Operation::PrintPosition,
            Operation::PrintWrite,
            Operation::PrintDraw,
            Operation::PrintFont,
            Operation::PrintNewPage,
            Operation::PrintFinish,
        ]
    );
    let draw = host
        .requests
        .iter()
        .find(|request| request.operation == Operation::Draw)
        .unwrap();
    assert_eq!(draw.window_cursor, (2, 3));
    assert_eq!(draw.arguments[0].1, HostValue::Object("AVIAO".into()));
    assert_eq!(draw.arguments[1].1, HostValue::Text("Frente".into()));
    let window_request = host
        .requests
        .iter()
        .find(|request| request.operation == Operation::ConfigureWindow)
        .unwrap();
    assert_eq!(window_request.window_size, (25, 80));
    assert_eq!(window_request.arguments.len(), 6);
    let menu = host
        .requests
        .iter()
        .find(|request| request.operation == Operation::Menu)
        .unwrap();
    assert_eq!(menu.rows[0][0].1, HostValue::Text("Um".into()));
}

#[test]
fn component_creation_and_value_requests_are_routed_to_the_host() {
    let mut host = TestHost {
        responses: VecDeque::from([
            HostValue::Object("aviao-1".into()),
            HostValue::Sound("som-1".into()),
            HostValue::Text("ignorado".into()),
            HostValue::Text("Ada".into()),
        ]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Aviao, Musica\nCria Objeto\nAviao = \"aviao.gif\"\nFimCria\nCria Som\nMusica = \"trilha.wav\"\nFimCria\nComponente \"Texto.Nome\", \"Lin:1; Col:2; Tam:10;\"\nComponente Valor, \"Texto.Nome\"\nEscreva Retorno\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "Ada");
    assert_eq!(
        host.requests
            .iter()
            .map(|request| request.operation)
            .collect::<Vec<_>>(),
        vec![
            Operation::CreateObject,
            Operation::CreateSound,
            Operation::Component,
            Operation::Component,
        ]
    );
    assert_eq!(
        host.requests[0].arguments[1].1,
        HostValue::Text("aviao.gif".into())
    );
    assert_eq!(
        host.requests[1].arguments[1].1,
        HostValue::Text("trilha.wav".into())
    );
    assert_eq!(
        host.requests[2].arguments[0].1,
        HostValue::Text("Texto.Nome".into())
    );
    assert_eq!(
        host.requests[3].arguments[0].1,
        HostValue::Text("Valor".into())
    );
    assert_eq!(
        host.requests[3].arguments[1].1,
        HostValue::Text("Texto.Nome".into())
    );

    let mut numeric_host = TestHost {
        responses: VecDeque::from([HostValue::Number(41.0)]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Idade\nComponente Valor, \"Moeda.Idade\"\nIdade = Retorno\nIdade++\nEscreva Idade\nFimPrograma",
        &mut numeric_host,
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "42");
}

#[test]
fn reserved_mouse_and_color_memories_are_forwarded_as_typed_values() {
    let mut host = TestHost {
        responses: VecDeque::from([
            HostValue::Number(10.0),
            HostValue::Number(20.0),
            HostValue::Logic(true),
            HostValue::Text("10".into()),
            HostValue::Color(0, 0, 255),
        ]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Linha, Coluna, Clicou, Situacao, Cor, CorNova\nLinha = LinhaMouse\nColuna = ColunaMouse\nClicou = CliqueMouse\nSituacao = TestaMouse\nCor = Azul\nCorNova = RGB(12, 34, 56)\nEscreva Linha, \":\", Coluna, \":\", Clicou, \":\", Situacao, \":\", Cor, \":\", CorNova\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    let window = result.unwrap().unwrap();
    assert_eq!(cell_text(&window, 0, 79), "10:20:SIM:10:#0000FF:#0C2238");
    assert_eq!(
        host.requests
            .iter()
            .map(|request| request.operation)
            .collect::<Vec<_>>(),
        vec![
            Operation::ReservedMemory,
            Operation::ReservedMemory,
            Operation::ReservedMemory,
            Operation::MouseButtons,
            Operation::ReservedMemory,
        ]
    );
}

#[test]
fn named_colors_reach_visual_host_operations_as_color_values() {
    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Color(255, 0, 0)]),
        ..TestHost::default()
    };
    let result = run_with_host("Fundo = Vermelho", &mut host);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        host.requests
            .iter()
            .map(|request| request.operation)
            .collect::<Vec<_>>(),
        vec![Operation::ReservedMemory, Operation::Background]
    );
    assert_eq!(host.requests[1].arguments[0].1, HostValue::Color(255, 0, 0));
}

#[test]
fn documented_fixed_arity_commands_reject_missing_or_extra_values() {
    let invalid_programs = [
        "Fundo = \"um.bmp\", \"dois.bmp\"",
        "Desenhe \"imagem.gif\", Frente, \"extra\"",
        "Som \"trilha.wav\", 0, 1",
        "FimSom",
        "Componente \"Texto.Nome\"",
        "BoxPosicao 1",
        "BoxTitulo \"Nome\"",
        "BoxAdd \"um\", \"dois\"",
        "BoxAtiva 1",
        "BoxNew 1",
        "BoxDesativa 1",
        "Imp Posicione 1",
        "Imp NovaPagina 1",
        "Imp Fim 1",
        "Memoria Hoje\nHoje = Data(1)",
        "Aguarde -1",
        "Se 1 = 1 E 2 = 2 OU 3 = 3",
    ];

    for source in invalid_programs {
        let mut host = TestHost::default();
        let result = run_with_host(source, &mut host);
        assert!(result.is_err(), "accepted invalid command: {source}");
        assert!(host.requests.is_empty(), "sent invalid command: {source}");
    }
}

#[test]
fn click_waits_return_the_selected_object_or_button_to_retorno() {
    let mut host = TestHost {
        responses: VecDeque::from([
            HostValue::Text("x".into()),
            HostValue::Text("Aviao".into()),
            HostValue::Text("Confirmar".into()),
        ]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Aguarde\nAguarde CliqueObjeto\nAguarde CliqueBotao\nEscreva Retorno\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "Confirmar");
    assert_eq!(
        host.requests
            .iter()
            .map(|request| request.operation)
            .collect::<Vec<_>>(),
        vec![
            Operation::WaitKey,
            Operation::WaitObject,
            Operation::WaitButton
        ]
    );
}

#[test]
fn fractional_aguarde_sends_one_numeric_duration_to_the_host() {
    let mut host = TestHost::default();
    let result = run_with_host("Aguarde 0,1\nFimPrograma", &mut host);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(host.requests.len(), 1);
    assert_eq!(host.requests[0].operation, Operation::WaitSeconds);
    assert_eq!(host.requests[0].arguments.len(), 1);
    assert_eq!(host.requests[0].arguments[0].1, HostValue::Number(0.1));
}

#[test]
fn menu_selection_is_returned_as_text() {
    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Number(1.0)]),
        ..TestHost::default()
    };
    let result = run_with_host("Menu \"Opções\"\n\"Um\"\nFimMenu", &mut host);

    assert!(result.is_err());

    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Text("Um".into())]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Menu \"Opções\"\n\"Um\"\nFimMenu\nEscreva Retorno\nFimPrograma",
        &mut host,
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(cell_text(&result.unwrap().unwrap(), 0, 79), "Um");
}

#[test]
fn menus_accept_ten_choices_and_reject_an_eleventh() {
    let choices = (1..=10)
        .map(|choice| format!("\"Opção {choice}\""))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("Menu \"Escolha\"\n{choices}\nFimMenu\nFimPrograma");
    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Text("Opção 10".into())]),
        ..TestHost::default()
    };

    let result = run_with_host(&source, &mut host);

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(host.requests[0].rows.len(), 10);

    let choices = (1..=11)
        .map(|choice| format!("\"Opção {choice}\""))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("Menu \"Escolha\"\n{choices}\nFimMenu\nFimPrograma");
    let mut host = TestHost::default();

    let result = run_with_host(&source, &mut host);

    assert!(result.is_err(), "accepted a menu with eleven choices");
    assert!(host.requests.is_empty());
}

#[test]
fn menu_without_a_selection_is_an_error() {
    let mut host = TestHost::default();
    let result = run_with_host("Menu \"Opções\"\n\"Um\"\nFimMenu\nFimPrograma", &mut host);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Menu did not return")
    );
    assert_eq!(host.requests.len(), 1);
    assert_eq!(host.requests[0].operation, Operation::Menu);
}

#[test]
fn database_commands_reject_invalid_argument_counts_before_host_calls() {
    let invalid_programs = [
        "AbreBD Items, \"file\", LinhaMouse",
        "FechaBD Items, LinhaMouse",
        "AdicionaRegistro Items, LinhaMouse",
        "LeRegistro Items, 0, LinhaMouse",
        "OrdemBD Items",
        "ExcluiRegistro Items, 0, LinhaMouse",
        "LimpaBD Items, LinhaMouse",
        "Memoria Posicao\nPosicao = TotalRegistros Items, LinhaMouse",
        "Memoria Posicao\nPosicao = Procure Items, Primeiro",
    ];

    for source in invalid_programs {
        let mut host = TestHost::default();
        let result = run_with_host(source, &mut host);

        assert!(result.is_err(), "accepted invalid source: {source}");
        assert!(
            host.requests.is_empty(),
            "evaluated host arguments before rejecting: {source}"
        );
    }
}

#[test]
fn random_and_text_functions_match_the_manual_examples() {
    let mut host = TestHost {
        responses: VecDeque::from([HostValue::Number(100.0)]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Sorteado, Numero, Formatado, Recorte\nSorteado = Aleatorio 100\nNumero = 1123,57\nFormatado = Formata Numero, \"#.##0,00\"\nRecorte = Separe \"Hoje é um dia chuvoso!\", 15, 7\nEscreva Sorteado, \"|\", Formatado, \"|\", Recorte\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        cell_text(&result.unwrap().unwrap(), 0, 79),
        "100|1.123,57|chuvoso"
    );
    assert_eq!(host.requests[0].operation, Operation::Random);
    assert_eq!(host.requests[0].arguments[0].1, HostValue::Number(100.0));

    let mut invalid_random = TestHost {
        responses: VecDeque::from([HostValue::Number(101.0)]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Numero\nNumero = Aleatorio 100",
        &mut invalid_random,
    );
    assert!(result.is_err());
}

#[test]
fn formata_matches_the_documented_masks_and_default_pattern() {
    let window = run(
        "Memoria X, Inteiro, DecimalFormatado, ResultadoPadrao\nX = 10\nInteiro = Formata X, \"00000\"\nX = \"123,5\"\nDecimalFormatado = Formata X, \"00000,00\"\nX = \"1234,5\"\nResultadoPadrao = Formata X, Padrao\nEscreva Inteiro, \"|\", DecimalFormatado, \"|\", ResultadoPadrao\nFimPrograma",
        |_, _| unreachable!("program does not request input"),
    )
    .unwrap()
    .unwrap();

    assert_eq!(cell_text(&window, 0, 79), "00010|00123,50|1.234,50");
}

#[test]
fn date_and_time_functions_forward_to_the_host_as_text_values() {
    let mut host = TestHost {
        responses: VecDeque::from([
            HostValue::Text("18/11/2026".into()),
            HostValue::Text("12:34:56".into()),
        ]),
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Hoje, Agora\nHoje = Data\nAgora = Hora\nEscreva Hoje, \" \", Agora\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        cell_text(&result.unwrap().unwrap(), 0, 79),
        "18/11/2026 12:34:56"
    );
    assert_eq!(
        host.requests
            .iter()
            .map(|request| request.operation)
            .collect::<Vec<_>>(),
        vec![Operation::Date, Operation::Time]
    );
}

#[test]
fn entre_is_not_guessed_without_a_documented_signature() {
    let error = run_with_host(
        "Memoria Resultado\nResultado = Entre(1, 2)",
        &mut TestHost::default(),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Entre's signature is not defined")
    );
}

#[test]
fn database_commands_support_ordered_search_delete_and_compaction() {
    let record = |name: &str, age: f64| {
        BTreeMap::from([
            ("Nome".to_owned(), HostValue::Text(name.to_owned())),
            ("Idade".to_owned(), HostValue::Number(age)),
        ])
    };
    let mut host = TestHost {
        database: vec![
            Some(record("Bia", 20.0)),
            Some(record("Ada", 30.0)),
            Some(record("Ada", 20.0)),
        ],
        ..TestHost::default()
    };
    let result = run_with_host(
        "Memoria Total, PosPrimeiro, PosUltimo, PosProximo\nBD Alunos \"dados\"\n\"Nome\", 10\n\"Idade\", 3\nFimBD\nAbreBD Alunos \"turma\"\nTotal = TotalRegistros Alunos\nOrdemBD Alunos, Nome, Idade\nPosPrimeiro = Procure Alunos, Primeiro, \"Ada\"\nPosUltimo = Procure Alunos, Ultimo, \"Ada\"\nPosProximo = Procure Alunos, Proximo, \"Ada\", 25\nLeRegistro Alunos PosPrimeiro\nEscreva Alunos.Nome, \" \", Alunos.Idade\nExcluiRegistro Alunos PosPrimeiro\nOrdemBD Alunos, Sequencial\nLimpaBD Alunos\nFechaBD Alunos\nEscreva Total, \":\", PosPrimeiro, \":\", PosUltimo, \":\", PosProximo\nFimPrograma",
        &mut host,
    );

    assert!(result.is_ok(), "{result:?}");
    let window = result.unwrap().unwrap();
    assert_eq!(cell_text(&window, 0, 79), "Ada 20");
    assert_eq!(cell_text(&window, 1, 79), "3:0:1:1");
    let (_, file, records) = host.saved_database.unwrap();
    assert_eq!(file, "turma.LBA");
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].as_ref().unwrap().get("Nome"),
        Some(&HostValue::Text("Bia".into()))
    );
    assert_eq!(
        records[1].as_ref().unwrap().get("Nome"),
        Some(&HostValue::Text("Ada".into()))
    );
    assert_eq!(host.closed_database.unwrap().1, "turma.LBA");
}
