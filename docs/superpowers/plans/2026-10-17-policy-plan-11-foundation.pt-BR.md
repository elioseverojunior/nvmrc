# nvmrc Plano 11: Plano de implementação da base da política de segurança

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use
> superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para implementar este plano tarefa por tarefa. Os
> passos usam a sintaxe de checkbox (`- [ ]`) para acompanhamento.

**Objetivo:** Entregar a etapa 1 do design da política de segurança: o arquivo
de política e seu JSON Schema, as flags `--policy` e `--min-node` com suas
variáveis, a política do repositório, a combinação que só aperta, os modos
legado e política, todas as regras do node exceto CVEs (`minimum`, `allow`,
`deny`, `ltsOnly`, `minimumReleaseAgeDays`) aplicadas em `install`, `use`,
`exec` e `run`, caminhos automáticos só com aviso, e o código de saída 9.

**Arquitetura:** A parte pura fica em `src/domain/policy/` (os tipos do
documento, as fontes e sua combinação que só aperta, o veredito, a tabela de
flags). A parte impura fica em `src/commands/policy/` (ler uma localização,
resolver o modo, o portão de aplicação que imprime e para). A CLI remove as
flags de política antes do clap e as entrega aos comandos por dois campos novos
do `Context`, `policy_flags` e `advisory`. Os comandos chamam um único portão,
`commands::policy::enforce::enforce_node`, depois que conhecem a versão.

**Stack:** Rust 2024 (MSRV 1.85, toolchain 1.99), dependências novas `serde` 1
(derive), `serde_json` 1, `nodejs-semver` 7 (Apache-2.0, MSRV 1.85),
dev-dependency `schemars` 1; cargo-deny, rumdl.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.pt-BR.md`
(seções 4, 5, 6.1 a 6.5, 6.10; etapa de entrega 1 da seção 7). Este plano parte
de `feat/policy-floor` em `3307f08`. Planos seguintes: 12 (checagem de CVE, com
`--fail-on-severity` e a regra `cve.source` da política do repositório), 13
(npm, yarn e pnpm, com `--min-npm`, `--min-yarn`, `--min-pnpm`), 14 (`nvm init`
no modo projeto), 15 (`nvm upgrade`). Este plano interpreta o schema inteiro
(incluindo `cve` e os blocos dos gerenciadores de pacotes) porque o schema é o
contrato, mas aplica só as regras do node.

## Restrições globais

- Sem política, sem flag `--policy`/`--min-node`, sem `NVMRC_POLICY` e sem
  `.nvmrc-policy.json` acima do diretório de trabalho, o comportamento é
  exatamente o de hoje; o `cargo test` (incluindo a suíte de compatibilidade)
  passa sem mudanças.
- `NVM_MIN_VERSION` ou `$NVM_DIR/min-version` sozinhos são o modo legado:
  checados só no `install`, com as mensagens de hoje.
- Toda falha em saber num comando explícito sai com **9**; toda violação sai
  com **7**. Caminhos automáticos (`__auto`) nunca mudam o status por causa da
  política: eles imprimem um aviso e seguem.
- Campos desconhecidos são rejeitados em todos os níveis de uma política;
  `schemaVersion` precisa ser `1`.
- Uma localização que começa com `http://` é rejeitada; `https://` passa pelo
  port `Http`; qualquer outra coisa é um caminho, e caminhos relativos são
  resolvidos a partir do diretório de trabalho lógico
  (`Context::working_directory`).
- Overrides só apertam: em `minimum` vence o mais alto, `allow` faz interseção,
  `deny` faz união, `ltsOnly` fica ligado se qualquer fonte ligar, em
  `minimumReleaseAgeDays` vence o maior; um override mais frouxo é um aviso que
  cita sua fonte e o valor que venceu.
- Variáveis de ambiente são lidas por `context.env`; a feature `env` do clap
  não é usada.
- Arquivos com menos de 300 linhas, funções com menos de 30, complexidade
  ciclomática abaixo de 10,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt`,
  `rumdl check` no Markdown alterado, sem rede nos testes, TDD.
- Commits seguem Conventional Commits, assinados com GPG (`git commit -S`), sem
  atribuição de IA.

## Foco da revisão

- `nvm exec 20 node --policy x.js`: um `--policy` entre os argumentos do
  processo filho chega intacto ao `node`; só são tomadas as flags antes do
  subcomando ou imediatamente depois do nome dele (teste da Tarefa 4
  `flags_after_the_first_argument_are_left_alone`).
- Uma política com vírgula final, comentário ou byte-order mark: saída 9 com a
  localização e a linha e coluna do serde (teste da Tarefa 1
  `malformed_json_reports_line_and_column`, teste da Tarefa 5
  `an_invalid_central_policy_is_a_failure_to_know`).
- `--policy ./sec.json` a partir de um subdiretório: resolvido a partir do
  diretório de trabalho lógico, não do diretório do processo (teste da Tarefa 5
  `a_relative_location_is_read_from_the_working_directory`).
- `NVMRC_POLICY=""` (definida, mas vazia): tratada como não definida, assim
  como o `NVM_MIN_VERSION` (teste da Tarefa 5
  `an_empty_policy_variable_is_unset`).
- `nvm use iojs-v3.3.1` no modo política: uma violação (o io.js não é coberto
  por nenhuma política), não um crash nem uma aprovação (teste da Tarefa 3
  `an_iojs_version_is_never_allowed`).

---

### Tarefa 1: Documento de política, interpretação e o JSON Schema

**Arquivos:**

- Modificar: `Cargo.toml` (dependências)
- Criar: `src/domain/policy/mod.rs`, `src/domain/policy/document.rs`,
  `src/domain/policy/document_tests.rs`
- Modificar: `src/domain/mod.rs` (adicionar `pub mod policy;`)
- Criar: `schemas/nvmrc-policy.v1.schema.json`

**Interfaces:**

- Produz:
  - `PolicyDocument { schema: Option<String> /* "$schema" */, schema_version:
    u32, tools: Tools, cve: Option<CveSettings> }`
  - `Tools { node: Option<NodeRules>, npm: Option<PackageRules>, yarn:
    Option<PackageRules>, pnpm: Option<PackageRules> }` (`Default`)
  - `NodeRules { minimum: Option<String>, allow: Option<String>, deny:
    Option<Vec<String>>, lts_only: Option<bool>, minimum_release_age_days:
    Option<u32> }`
  - `PackageRules` — os mesmos campos, sem `lts_only`
  - `CveSettings { fail_on_severity: Option<Severity>, source: Option<String> }`
  - `enum Severity { Critical, High, Medium, Low, None }` (minúsculas no JSON)
  - `pub fn parse_document(text: &str) -> Result<PolicyDocument,
    DocumentError>`
  - `enum DocumentError { Malformed(String), UnsupportedSchemaVersion(u32),
    InvalidVersion { field: String, value: String }, InvalidRange { field:
    String, value: String } }` (thiserror)

- [ ] **Passo 1: Adicionar as dependências**

`Cargo.toml`: `serde = { version = "1", features = ["derive"] }`,
`serde_json = "1"`, `nodejs-semver = "7"`; dev: `schemars = "1"`. Rode
`cargo deny check` — esperado: nenhum erro de licença ou de origem.

- [ ] **Passo 2: Escrever os testes que falham** em `document_tests.rs`

```rust
#[test]
fn the_spec_example_parses() {
    let document = parse_document(SPEC_EXAMPLE).unwrap(); // §6.1 example, verbatim
    let node = document.tools.node.unwrap();
    assert_eq!(node.minimum.as_deref(), Some("20.11.1"));
    assert_eq!(node.allow.as_deref(), Some(">=20 <25"));
    assert_eq!(node.deny, Some(vec!["20.12.0".into(), "22.3.0".into()]));
    assert_eq!(node.lts_only, Some(true));
    assert_eq!(node.minimum_release_age_days, Some(14));
    assert_eq!(document.cve.unwrap().fail_on_severity, Some(Severity::High));
}

#[test]
fn an_unknown_field_is_rejected_at_any_level() {
    for text in [
        r#"{"schemaVersion":1,"extra":true}"#,
        r#"{"schemaVersion":1,"tools":{"deno":{}}}"#,
        r#"{"schemaVersion":1,"tools":{"node":{"minimun":"20"}}}"#,
        r#"{"schemaVersion":1,"cve":{"failOn":"high"}}"#,
    ] {
        assert!(matches!(parse_document(text), Err(DocumentError::Malformed(_))), "{text}");
    }
}

#[test]
fn lts_only_is_rejected_outside_node() {
    let text = r#"{"schemaVersion":1,"tools":{"pnpm":{"ltsOnly":true}}}"#;
    assert!(matches!(parse_document(text), Err(DocumentError::Malformed(_))));
}

#[test]
fn only_schema_version_1_is_understood() {
    assert_eq!(parse_document(r#"{"schemaVersion":2}"#),
        Err(DocumentError::UnsupportedSchemaVersion(2)));
    assert!(matches!(parse_document("{}"), Err(DocumentError::Malformed(_))));
}

#[test]
fn versions_and_ranges_are_validated() {
    let bad_minimum = r#"{"schemaVersion":1,"tools":{"node":{"minimum":"twenty"}}}"#;
    let bad_deny = r#"{"schemaVersion":1,"tools":{"node":{"deny":["20"]}}}"#; // deny is exact only
    let bad_range = r#"{"schemaVersion":1,"tools":{"node":{"allow":">>20"}}}"#;
    assert!(matches!(parse_document(bad_minimum), Err(DocumentError::InvalidVersion { .. })));
    assert!(matches!(parse_document(bad_deny), Err(DocumentError::InvalidVersion { .. })));
    assert!(matches!(parse_document(bad_range), Err(DocumentError::InvalidRange { .. })));
}

#[test]
fn malformed_json_reports_line_and_column() {
    let error = parse_document("{\n  \"schemaVersion\": 1,\n}").unwrap_err();
    assert!(error.to_string().contains("line 3"), "{error}");
}

#[test]
fn committed_schema_matches_the_types() {
    let generated = serde_json::to_string_pretty(&schemars::schema_for!(PolicyDocument)).unwrap();
    let committed = include_str!("../../../schemas/nvmrc-policy.v1.schema.json");
    assert_eq!(committed.trim_end(), generated, "regenerate schemas/nvmrc-policy.v1.schema.json");
}
```

- [ ] **Passo 3: Rodá-los** — `cargo test domain::policy::document` —
  esperado: falha de compilação (módulo ausente).

- [ ] **Passo 4: Implementar `document.rs`**

Os tipos derivam `Debug, Clone, PartialEq, Eq, Deserialize` e
`#[cfg_attr(test, derive(schemars::JsonSchema))]`, com
`#[serde(deny_unknown_fields, rename_all = "camelCase")]`; `Severity` usa
`rename_all = "lowercase"`. `parse_document` desserializa com
`serde_json::from_str` (mapeie o erro para `Malformed(error.to_string())`, que
traz linha e coluna) e depois valida: `schema_version == 1`; todo `minimum` é
interpretável como `VersionPattern`; toda entrada de `deny` é interpretável
como `Version` e tem três partes numéricas; todo `allow` é interpretável como
`nodejs_semver::Range`. `field` cita o caminho JSON (`tools.node.deny[0]`).
Adicione anotações de schema para que o schema diga o que o código aplica:
`schemaVersion` com mínimo e máximo 1, strings de versão com o pattern
`^v?\d+(\.\d+){0,2}$`, itens de `deny` com o pattern `^v?\d+\.\d+\.\d+$`.

- [ ] **Passo 5: Escrever o arquivo de schema** — rode o teste do schema uma
  vez, cole o JSON gerado (do lado direito da asserção) em
  `schemas/nvmrc-policy.v1.schema.json` com uma quebra de linha final,
  adicione `"$id":
  "https://raw.githubusercontent.com/elioseverojunior/nvmrc/main/schemas/nvmrc-policy.v1.schema.json"`
  por um atributo `#[schemars(extend("$id" = ...))]` em `PolicyDocument`, para
  que o arquivo continue gerado, e rode de novo.

- [ ] **Passo 6: Rodar os testes** — `cargo test domain::policy::document` —
  esperado: todos passam.

- [ ] **Passo 7: Commit** — `feat(policy): parse and validate the policy
  document and publish its schema`

### Tarefa 2: Fontes e a combinação que só aperta

**Arquivos:**

- Criar: `src/domain/policy/source.rs`, `src/domain/policy/rules.rs`,
  `src/domain/policy/rules_tests.rs`

**Interfaces:**

- Consome: `NodeRules` (Tarefa 1), `Version`, `VersionPattern`.
- Produz:
  - `enum Source { Flag(&'static str), Variable(&'static str),
    CentralPolicy(String), RepositoryPolicy(PathBuf), MinVersionFile(PathBuf) }`
    com `Display` (`--min-node`, `NVM_MIN_VERSION`, `policy
    https://sec/p.json`, `repository policy /w/.nvmrc-policy.json`,
    `/n/min-version`) e `fn is_override(&self) -> bool` (todas menos
    `CentralPolicy`)
  - `struct Sourced<T> { pub value: T, pub source: Source }`
  - `struct NodeLayer { pub source: Source, pub minimum: Option<Version>, pub
    allow: Option<nodejs_semver::Range>, pub allow_text: Option<String>, pub
    deny: Vec<Version>, pub lts_only: Option<bool>,
    pub minimum_release_age_days: Option<u32> }`
  - `impl NodeLayer { pub fn from_rules(source: Source, rules: &NodeRules) ->
    Result<Self, DocumentError>; pub fn minimum_only(source: Source, minimum:
    Version) -> Self }` (`from_rules` reinterpreta as strings que
    `parse_document` já validou; ele retorna o erro em vez de usar unwrap)
  - `struct NodePolicy { pub minimum: Option<Sourced<Version>>, pub allow:
    Vec<Sourced<(nodejs_semver::Range, String)>>, pub deny:
    Vec<Sourced<Version>>, pub lts_only: Option<Source>,
    pub minimum_release_age_days: Option<Sourced<u32>> }`
  - `pub fn merge(layers: Vec<NodeLayer>) -> (NodePolicy, Vec<String>)` — a
    política e os avisos de afrouxamento
- [ ] **Passo 1: Escrever os testes que falham** em `rules_tests.rs`

```rust
#[test]
fn the_highest_minimum_wins_and_names_its_source() {
    let (policy, _) = merge(vec![central(minimum("20.11.1")), variable(minimum("22"))]);
    let winner = policy.minimum.unwrap();
    assert_eq!(winner.value.to_string(), "v22.0.0");
    assert_eq!(winner.source, Source::Variable("NVM_MIN_VERSION"));
}

#[test]
fn a_lower_override_loses_with_a_warning() {
    let (policy, warnings) = merge(vec![central(minimum("20.11.1")), flag(minimum("16"))]);
    assert_eq!(policy.minimum.unwrap().value.to_string(), "v20.11.1");
    assert_eq!(warnings, vec![
        "nvm: --min-node 16 is looser than v20.11.1 from policy /p.json; using v20.11.1."
    ]);
}

#[test]
fn allow_ranges_all_apply_and_deny_lists_add_up() {
    let (policy, _) = merge(vec![central(allow(">=20 <25").deny(&["20.12.0"])),
                                 repository(allow("^22").deny(&["22.3.0"]))]);
    assert_eq!(policy.allow.len(), 2);
    assert_eq!(policy.deny.len(), 2);
}

#[test]
fn lts_only_is_on_when_any_source_sets_it_and_an_explicit_false_warns() {
    let (policy, warnings) = merge(vec![central(lts_only(true)), repository(lts_only(false))]);
    assert!(policy.lts_only.is_some());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_largest_release_age_wins() {
    let (policy, warnings) = merge(vec![central(age(14)), repository(age(7))]);
    assert_eq!(policy.minimum_release_age_days.unwrap().value, 14);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_central_policy_never_warns_about_itself() {
    let (_, warnings) = merge(vec![central(minimum("18")), repository(minimum("20"))]);
    assert!(warnings.is_empty());
}
```

(`central`, `flag`, `variable`, `repository` e os construtores de regras são
pequenos helpers de teste no arquivo de testes que retornam `NodeLayer`;
`central` usa `Source::CentralPolicy("/p.json".into())`, `flag` usa
`Source::Flag("--min-node")`.)

- [ ] **Passo 2: Rodá-los** — `cargo test domain::policy::rules` — esperado:
  falha de compilação.

- [ ] **Passo 3: Implementar** `source.rs` e `rules.rs`. Texto do aviso,
  exato: `nvm: {source} {value} is looser than {winner} from {winner_source};
  using {winner}.` (para `ltsOnly`: valor `false`, vencedor `true`; para a
  idade: contagens de dias). Só uma fonte de override (`is_override`) que perde
  produz um aviso. `allow` guarda o texto original para as mensagens.

- [ ] **Passo 4: Rodar os testes** — esperado: passam.

- [ ] **Passo 5: Commit** — `feat(policy): merge rule sources so overrides only
  tighten`

### Tarefa 3: O veredito, com as datas de release do índice

**Arquivos:**

- Criar: `src/domain/policy/verdict.rs`, `src/domain/policy/verdict_tests.rs`
- Modificar: `src/domain/index.rs` (`Release` ganha `published_day`),
  `src/domain/timestamp.rs` (`days_from_iso_date`),
  `src/domain/fixtures.rs` (`index_text_dated`)

**Interfaces:**

- Consome: `NodePolicy`, `Sourced`, `Source` (Tarefa 2), `Version`, `Flavor`.
- Produz:
  - `Release { version, lts, pub published_day: Option<i64> }` — dias desde
    1970-01-01 a partir da coluna 2 (`YYYY-MM-DD`); `None` quando ausente ou
    inválida
  - `pub fn days_from_iso_date(text: &str) -> Option<i64>` (inversa da
    `civil_from_days` existente)
  - `pub fn index_text_dated(rows: &[(&str, &str, &str)]) -> String` —
    `(version, date, lts)`
  - `struct ReleaseInfo { pub lts: Option<String>, pub published_day:
    Option<i64> }`
  - `enum Violation { BelowMinimum(Sourced<Version>), OutsideRange(Sourced<String>),
    Denied(Source), NotLts(Source), TooRecent { age_days: i64, required:
    Sourced<u32> }, NotNode }` com `Display` gerando o texto do item da lista
  - `impl NodePolicy { pub fn needs_release_info(&self) -> bool; pub fn
    violations(&self, version: &Version, release: Option<&ReleaseInfo>,
    today_day: i64) -> Result<Vec<Violation>, MissingReleaseInfo> }`
  - `struct MissingReleaseInfo;` (a versão não está no índice, embora uma regra
    precise dela)
- [ ] **Passo 1: Escrever os testes que falham**

```rust
// timestamp.rs tests
#[test]
fn iso_dates_round_trip_through_days() {
    assert_eq!(days_from_iso_date("1970-01-01"), Some(0));
    assert_eq!(days_from_iso_date("2024-02-29"), Some(19_782));
    assert_eq!(days_from_iso_date("2024-02-30"), None);
    assert_eq!(days_from_iso_date("-"), None);
}

// index.rs tests
#[test]
fn the_release_date_is_read_from_column_two() {
    let text = index_text_dated(&[("v22.11.0", "2024-10-29", "Jod")]);
    assert_eq!(parse_index(&text, Flavor::Node)[0].published_day, days_from_iso_date("2024-10-29"));
}

// verdict_tests.rs
#[test]
fn each_rule_reports_its_own_violation() {
    let policy = policy_with_all_rules(); // min 20.11.1, allow ">=20 <25", deny 20.12.0, ltsOnly, 14 days
    let today = days_from_iso_date("2024-11-01").unwrap();
    let release = ReleaseInfo { lts: None, published_day: days_from_iso_date("2024-10-29") };
    let found = policy.violations(&v("v25.1.0"), Some(&release), today).unwrap();
    let text: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(text, vec![
        "outside the allowed range >=20 <25 (from policy /p.json)",
        "not an LTS release (from policy /p.json)",
        "published 3 days ago; at least 14 required (from policy /p.json)",
    ]);
    assert_eq!(policy.violations(&v("v18.0.0"), Some(&lts("Hydrogen")), today).unwrap()[0].to_string(),
        "below the minimum v20.11.1 (from policy /p.json)");
    assert_eq!(policy.violations(&v("v20.12.0"), Some(&lts("Iron")), today).unwrap()[0].to_string(),
        "denied (from policy /p.json)");
}

#[test]
fn an_allowed_version_has_no_violations() {
    let today = days_from_iso_date("2024-11-01").unwrap();
    let old_lts = ReleaseInfo { lts: Some("Iron".into()), published_day: days_from_iso_date("2024-07-08") };
    assert!(policy_with_all_rules().violations(&v("v20.15.1"), Some(&old_lts), today).unwrap().is_empty());
}

#[test]
fn release_info_is_needed_only_for_lts_and_age_rules() {
    assert!(!minimum_only("20").needs_release_info());
    assert!(policy_with_all_rules().needs_release_info());
    assert_eq!(policy_with_all_rules().violations(&v("v20.15.1"), None, 0), Err(MissingReleaseInfo));
}

#[test]
fn an_iojs_version_is_never_allowed() {
    let found = minimum_only("1").violations(&v("iojs-v3.3.1"), None, 0).unwrap();
    assert_eq!(found, vec![Violation::NotNode]);
    assert_eq!(found[0].to_string(), "io.js is not covered by policy");
}
```

- [ ] **Passo 2: Rodá-los** — esperado: falha de compilação.

- [ ] **Passo 3: Implementar** `days_from_iso_date` (o `days_from_civil` de
  Hinnant, rejeitando datas impossíveis com uma ida e volta por
  `civil_from_days`), `published_day` em `parse_row`, `index_text_dated` e
  `verdict.rs`. Ordem das violações: `NotNode` sozinha (para), depois mínimo,
  faixa, deny, LTS, idade. A idade é `today_day - published_day`; uma release
  sem data sob uma regra de idade é `MissingReleaseInfo`. As checagens de faixa
  convertem `Version` em `nodejs_semver::Version` pelo seu texto
  `major.minor.patch`.

- [ ] **Passo 4: Rodar** `cargo test domain::` — esperado: passam (testes de
  índice existentes sem mudanças).

- [ ] **Passo 5: Commit** — `feat(policy): decide whether a node version is
  allowed`

### Tarefa 4: Flags de política na CLI e no contexto

**Arquivos:**

- Criar: `src/domain/policy/flags.rs`, `src/domain/policy/flags_tests.rs`
- Modificar: `src/context/mod.rs` (derivar `Clone`; campos `policy_flags`,
  `advisory`), `src/cli/mod.rs` (`run` remove as flags),
  `src/cli/tests/policy.rs` (novo), `src/cli/tests/mod.rs` (`mod policy;`)

**Interfaces:**

- Produz:
  - `struct PolicyFlag { pub name: &'static str, pub variable: &'static str }`
  - `pub const POLICY_FLAGS: [PolicyFlag; 2] = [{ "--policy", "NVMRC_POLICY" },
    { "--min-node", "NVM_MIN_VERSION" }]` — planos seguintes acrescentam a esta
    tabela
  - `#[derive(Debug, Clone, Default, PartialEq, Eq)] struct PolicyFlags` com
    `pub fn get(&self, name: &str) -> Option<&str>` e `pub fn is_empty(&self)
    -> bool`
  - `pub fn extract(args: Vec<OsString>) -> Result<(PolicyFlags,
    Vec<OsString>), FlagError>`; mensagens do `FlagError` (thiserror)
    exatamente `nvm: {flag} needs a value.`, `nvm: {flag} is given twice.`,
    `nvm: the value of {flag} is not valid UTF-8.`
  - `Context::with_policy_flags(self, PolicyFlags) -> Self`,
    `Context::policy_flags(&self) -> &PolicyFlags`, `Context::advisory(self)
    -> Self`, `Context::is_advisory(&self) -> bool`

- [ ] **Passo 1: Escrever os testes que falham** em `flags_tests.rs`

```rust
#[test]
fn flags_before_the_subcommand_are_taken() {
    let (flags, rest) = extract(args(&["nvm", "--policy", "/p.json", "use", "20"])).unwrap();
    assert_eq!(flags.get("--policy"), Some("/p.json"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
}

#[test]
fn flags_right_after_the_subcommand_are_taken_in_both_forms() {
    let (flags, rest) = extract(args(&["nvm", "use", "--min-node=20", "--policy", "p", "20"])).unwrap();
    assert_eq!(flags.get("--min-node"), Some("20"));
    assert_eq!(flags.get("--policy"), Some("p"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
}

#[test]
fn flags_after_the_first_argument_are_left_alone() {
    let (flags, rest) = extract(args(&["nvm", "exec", "20", "node", "--policy", "x.js"])).unwrap();
    assert!(flags.is_empty());
    assert_eq!(rest, args(&["nvm", "exec", "20", "node", "--policy", "x.js"]));
}

#[test]
fn a_missing_or_repeated_value_is_an_error() {
    assert_eq!(extract(args(&["nvm", "--policy"])).unwrap_err().to_string(), "nvm: --policy needs a value.");
    assert_eq!(extract(args(&["nvm", "--policy=a", "--policy", "b", "ls"])).unwrap_err().to_string(),
        "nvm: --policy is given twice.");
}
```

Em `src/cli/tests/policy.rs`:

```rust
#[test]
fn a_flag_error_is_a_usage_error() {
    let (code, _, err) = run_cli(&["nvm", "--policy"]);
    assert_eq!(code, 127);
    assert_eq!(err.trim_end(), "nvm: --policy needs a value.");
}

#[test]
fn stripped_flags_do_not_reach_clap() {
    let (code, out, _) = run_cli(&["nvm", "--min-node", "18", "version", "20"]);
    assert_eq!((code, out.trim_end()), (0, "v20.1.0"));
}
```

- [ ] **Passo 2: Rodá-los** — esperado: falha de compilação.

- [ ] **Passo 3: Implementar** `extract`: a partir do índice 1, tome flags de
  política enquanto elas vierem; mantenha o próximo argumento (o subcomando);
  depois tome flags de política de novo enquanto elas vierem; copie todo o
  resto literalmente. Em `cli::run`, chame-o nos argumentos brutos **antes** de
  `arguments(...)`, para que um `nvm --policy p` puro continue virando
  `nvm --help` e a regra do `--` de `keep_leading_double_dash` veja o
  subcomando real no índice 1; em caso de erro, imprima a mensagem e retorne
  127; caso contrário, rode o clap em `arguments(rest)` com
  `context.clone().with_policy_flags(flags)`. `PolicyFlags`, `FlagError`,
  `PolicyError` e `MissingReleaseInfo` derivam `PartialEq` para os testes.

- [ ] **Passo 4: Rodar** `cargo test` — esperado: todos passam.

- [ ] **Passo 5: Commit** — `feat(policy): accept --policy and --min-node
  around the subcommand`

### Tarefa 5: Ler localizações e resolver o modo

**Arquivos:**

- Criar: `src/commands/policy/mod.rs`, `src/commands/policy/load.rs`,
  `src/commands/policy/mode.rs`, `src/commands/policy/tests/` (`load.rs`,
  `mode.rs`, `mod.rs`)
- Modificar: `src/commands/mod.rs` (`pub mod policy;`),
  `src/domain/nvmrc/mod.rs` (generalizar a subida de diretórios em
  `pub fn find_up(fs, start, file_name) -> Option<PathBuf>`; `find_nvmrc` a
  chama), `src/error.rs` (`PolicyError`, `CliError::Policy`,
  `NvmExitCode::PolicyUnverifiable` = 9)

**Interfaces:**

- Consome: `parse_document`, `NodeLayer`, `merge`, `Source` (Tarefas 1–2),
  `Context::policy_flags` (Tarefa 4), `VersionFloor::parse`.
- Produz:
  - `pub const REPOSITORY_POLICY_FILE: &str = ".nvmrc-policy.json";`
  - `pub fn read_location(context: &Context<'_>, location: &str) ->
    Result<String, PolicyError>`
  - `pub fn policy_mode_requested(context: &Context<'_>) -> bool` — uma flag de
    política, um `NVMRC_POLICY` não vazio ou um arquivo de política do
    repositório encontrado
  - `pub enum Mode { Legacy, Enforced(NodePolicy) }`
  - `pub fn resolve_mode(context: &Context<'_>) -> Result<(Mode, Vec<String>),
    PolicyError>`
  - `enum PolicyError { InsecureUrl(String), Unreadable { location: String,
    reason: String }, Invalid { location: String, reason: String },
    ReleaseInfoUnavailable(String) }` — as mensagens começam com
    `nvm: cannot check the policy:` e citam a localização
- [ ] **Passo 1: Escrever os testes que falham**

```rust
#[test]
fn an_http_location_is_rejected() {
    assert_eq!(read_location(&ctx(), "http://sec/p.json"), Err(PolicyError::InsecureUrl("http://sec/p.json".into())));
}

#[test]
fn an_https_location_is_fetched_and_a_failure_is_unreadable() {
    let http = FakeHttp::default().with_body("https://sec/p.json", POLICY);
    assert_eq!(read_location(&ctx().with_http(&http), "https://sec/p.json").unwrap(), POLICY);
    assert!(matches!(read_location(&ctx(), "https://sec/missing.json"), Err(PolicyError::Unreadable { .. })));
}

#[test]
fn a_relative_location_is_read_from_the_working_directory() {
    // FakeEnv PWD=/w/app (with current dir /w/app), file /w/app/sec/p.json
    assert_eq!(read_location(&ctx_in("/w/app"), "sec/p.json").unwrap(), POLICY);
}

#[test]
fn legacy_mode_without_any_new_configuration() {
    // NVM_MIN_VERSION=20 and $NVM_DIR/min-version set, nothing else
    assert!(matches!(resolve_mode(&legacy_ctx()).unwrap().0, Mode::Legacy));
}

#[test]
fn an_empty_policy_variable_is_unset() {
    assert!(!policy_mode_requested(&ctx_with_var("NVMRC_POLICY", "")));
}

#[test]
fn the_min_node_flag_alone_enforces() {
    let Mode::Enforced(policy) = resolve_mode(&ctx_with_flag("--min-node", "20")).unwrap().0 else { panic!() };
    assert_eq!(policy.minimum.unwrap().source, Source::Flag("--min-node"));
}

#[test]
fn a_repository_policy_is_found_above_the_working_directory_and_enforces() {
    // /w/.nvmrc-policy.json with node.minimum 22, PWD=/w/app/src
    let Mode::Enforced(policy) = resolve_mode(&repo_ctx()).unwrap().0 else { panic!() };
    assert_eq!(policy.minimum.unwrap().source, Source::RepositoryPolicy("/w/.nvmrc-policy.json".into()));
}

#[test]
fn an_invalid_central_policy_is_a_failure_to_know() {
    let error = resolve_mode(&ctx_with_var("NVMRC_POLICY", "/bad.json")).unwrap_err(); // "{ \"schemaVersion\": 1, }"
    assert!(matches!(error, PolicyError::Invalid { .. }));
    assert_eq!(CliError::from(error).exit_code().code(), 9);
}

#[test]
fn every_source_takes_part_in_the_merge() {
    // central minimum 20, NVM_MIN_VERSION 22, min-version file 18, flag --min-node 21
    let (Mode::Enforced(policy), warnings) = resolve_mode(&all_sources_ctx()).unwrap() else { panic!() };
    assert_eq!(policy.minimum.unwrap().value.to_string(), "v22.0.0");
    assert_eq!(warnings.len(), 2); // the flag and the file are looser
}
```

- [ ] **Passo 2: Rodá-los** — esperado: falha de compilação.

- [ ] **Passo 3: Implementar.** Ordem das camadas passadas ao `merge`: flag
  (`--min-node`), variável (`NVM_MIN_VERSION`), política central (valor da flag
  `--policy`, senão `NVMRC_POLICY`; `Source::CentralPolicy(location)`),
  política do repositório (`find_up` a partir de `working_directory`), arquivo
  min-version (primeira linha, `Source::MinVersionFile`). Valores vazios são
  tratados como não definidos. Uma flag ou variável que não é uma versão é
  `PolicyError::Invalid` citando-a. Mapeie `PolicyError` para a saída 9 em
  `CliError::exit_code` e adicione 9 ao teste
  `exit_codes_match_the_nvm_contract`.

- [ ] **Passo 4: Rodar** `cargo test` — esperado: passam.

- [ ] **Passo 5: Commit** — `feat(policy): load central and repository
  policies and resolve the mode`

### Tarefa 6: O portão de aplicação e o `install`

**Arquivos:**

- Criar: `src/commands/policy/enforce.rs`,
  `src/commands/policy/tests/enforce.rs`
- Modificar: `src/commands/install/mod.rs:57` (chamar o portão),
  `src/commands/install/tests/` (novos casos de política)

**Interfaces:**

- Consome: `resolve_mode`, `policy_mode_requested`, `Mode`, `PolicyError`
  (Tarefa 5), `NodePolicy::violations`, `ReleaseInfo` (Tarefa 3),
  `remote_index::fetch`, `Context::is_advisory` (Tarefa 4).
- Produz:
  - `pub enum Origin { Nvmrc(PathBuf), Argument, System }`
  - `pub struct Requested { pub version: Version, pub origin: Origin, pub
    offline: bool }`
  - `pub enum Gate { Pass, Stop(NvmExitCode) }`
  - `pub fn enforce_node(context: &Context<'_>, requested: &Requested,
    transcript: &mut Transcript) -> Result<Gate, CliError>`
- [ ] **Passo 1: Escrever os testes que falham** (`tests/enforce.rs`)

```rust
#[test]
fn legacy_mode_always_passes() {
    assert!(matches!(gate(&legacy_ctx(), "v16.0.0"), (Gate::Pass, _)));
}

#[test]
fn a_violation_stops_with_7_and_names_everything() {
    let (gate, transcript) = gate_from_nvmrc(&min_ctx("20.11.1"), "v18.19.0", "/w/app/.nvmrc");
    assert!(matches!(gate, Gate::Stop(NvmExitCode::BelowVersionFloor)));
    assert_eq!(stderr(transcript), [
        "nvm: node v18.19.0 (from /w/app/.nvmrc) is not allowed by policy:",
        "  - below the minimum v20.11.1 (from --min-node)",
        "Raise /w/app/.nvmrc to an allowed version.",
    ].join("\n"));
}

#[test]
fn an_advisory_context_warns_and_passes() {
    let (gate, transcript) = gate(&min_ctx("20.11.1").advisory(), "v18.19.0");
    assert!(matches!(gate, Gate::Pass));
    assert!(stderr(transcript).starts_with("nvm: warning: node v18.19.0 is not allowed by policy:"));
}

#[test]
fn rules_needing_release_info_fetch_the_index_once() {
    // ltsOnly policy, FakeHttp serving index_text_dated at https://nodejs.org/dist/index.tab
    let (gate, _) = gate(&lts_ctx_with_index(), "v23.1.0");
    assert!(matches!(gate, Gate::Stop(_)));
    assert_eq!(http_requests().len(), 1);
}

#[test]
fn an_unreachable_index_is_a_failure_to_know() {
    let error = enforce_node(&lts_ctx_without_index(), &requested("v20.15.1"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}

#[test]
fn offline_with_a_release_rule_is_a_failure_to_know() {
    let error = enforce_node(&lts_ctx_with_index(), &offline("v20.15.1"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}
```

Em `src/commands/install/tests/`:

```rust
#[test]
fn install_below_a_policy_minimum_stops_before_downloading() { /* --min-node 22 via flags, install 20 → status 7, no tarball request */ }

#[test]
fn install_in_legacy_mode_keeps_todays_floor_message() { /* NVM_MIN_VERSION=22 only → today's two lines, status 7 */ }
```

- [ ] **Passo 2: Rodá-los** — esperado: falha de compilação.

- [ ] **Passo 3: Implementar** `enforce_node`: legado → `Pass`; resolva o modo
  (imprima seus avisos); busque informações de release só quando
  `needs_release_info()` (offline → `ReleaseInfoUnavailable("offline")`; sem
  releases ou versão não listada → `ReleaseInfoUnavailable(<index url>)`); hoje
  é `context.clock().unix_seconds().div_euclid(86_400)`. Violações: no modo
  bloqueante, imprime o cabeçalho, uma linha `-` por violação e a linha de
  correção, depois `Stop(BelowVersionFloor)`; no modo consultivo, prefixa o
  cabeçalho com `nvm: warning:` e passa. Um `PolicyError` bloqueia como
  `Err(CliError::Policy)` ou, no modo consultivo, imprime
  `nvm: warning: {error}` e passa. Texto da origem: `(from <path>)` para o
  `.nvmrc`, `(from the command line)` para um argumento, `(system)` para o node
  do sistema; a linha de correção cita o `.nvmrc` quando existe um, senão
  `Use an allowed version.` No `install`, substitua `check_floor(...)?` por:
  quando `policy_mode_requested`, chame o portão (um `Stop(status)` vira
  `Halt::Exit(status)`), senão o `check_floor` existente.

- [ ] **Passo 4: Rodar** `cargo test` — esperado: passam.

- [ ] **Passo 5: Commit** — `feat(policy): enforce node rules in install`

### Tarefa 7: `use`, `exec`, `run` e os caminhos automáticos

**Arquivos:**

- Modificar: `src/commands/use_version/mod.rs` (`run_and_target`),
  `src/commands/exec/launch.rs` (`launch`), `src/commands/auto/mod.rs`
  (contexto consultivo)
- Testar: `src/commands/use_version/tests.rs`, `src/commands/exec/tests/`,
  `src/commands/auto/tests/`, `src/commands/run/tests.rs`

**Interfaces:**

- Consome: `enforce_node`, `Requested`, `Origin`, `Gate` (Tarefa 6),
  `commands::resolve::system_version`, `Context::advisory` (Tarefa 4).

- [ ] **Passo 1: Escrever os testes que falham**

```rust
// use_version/tests.rs
#[test]
fn use_of_a_denied_installed_version_stops_without_switching() {
    // installed v20.12.0, repository policy deny ["20.12.0"]
    let output = run(&ctx, &["20.12.0".into()]).unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(output.script.is_none());
}

#[test]
fn use_system_checks_the_system_node_version() {
    // system node prints v18.0.0, --min-node 20
    assert_eq!(run(&ctx, &["system".into()]).unwrap().status, NvmExitCode::BelowVersionFloor);
}

#[test]
fn use_without_policy_is_unchanged() { /* existing fixture, no policy → same output as before */ }

// exec/tests
#[test]
fn exec_with_a_disallowed_version_runs_nothing() {
    let output = exec::run(&ctx_min_22, &args(&["20", "node", "-v"])).unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(output.spawn.is_none());
}

// run/tests.rs
#[test]
fn run_goes_through_the_same_gate() { /* nvm run 20 with --min-node 22 → 7, no spawn */ }

// auto/tests
#[test]
fn auto_use_warns_but_still_switches() {
    // default alias → v20.1.0, --min-node 22
    let output = auto::run(&ctx, &["use".into()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(output.script.is_some());
    assert!(output.stderr.contains("nvm: warning: node v20.1.0 is not allowed by policy:"));
}

#[test]
fn auto_use_with_an_unreadable_policy_warns_and_switches() { /* NVMRC_POLICY=/missing.json → status 0, warning */ }
```

- [ ] **Passo 2: Rodá-los** — esperado: os casos novos falham.

- [ ] **Passo 3: Implementar.** `use`: depois que `target::resolve` tem sucesso
  e antes de `apply`, monte `Requested` a partir do alvo (`Installed` → sua
  versão, `Origin::Nvmrc(find_nvmrc(...))` quando `from_nvmrc`, senão
  `Argument`; `System` → `system_version` interpretado como `Version`,
  `Origin::System`; nenhuma versão legível do sistema com o modo política
  ligado é `PolicyError::ReleaseInfoUnavailable("system node version")`); um
  `Stop` encerra o transcript com aquele status. `exec`: o mesmo portão em
  `launch`, assim que `nvm_version` der `Version` ou `System`, antes da linha
  `Running`. `run` não precisa de mudança além do seu teste (ele delega ao
  `exec`). `auto`: `auto_use` e `auto_install` passam
  `&context.clone().advisory()` para `use_version::run` e `install::run`.

- [ ] **Passo 4: Rodar** `cargo test` — esperado: passam, incluindo a suíte de
  compatibilidade.

- [ ] **Passo 5: Commit** — `feat(policy): enforce node rules in use, exec and
  run; warn on automatic paths`

### Tarefa 8: A função de shell pula as flags de política iniciais

**Arquivos:**

- Modificar: `src/shell/init/posix.rs`, `src/shell/init/fish.rs`,
  `src/shell/init/mod.rs` (um helper que monta os patterns a partir de
  `POLICY_FLAGS`)
- Testar: testes unitários de `src/shell/init/`, `tests/init_lab` (shells
  reais)

**Interfaces:**

- Consome: `POLICY_FLAGS` (Tarefa 4).
- Produz: `pub(super) fn policy_flag_patterns() -> (String, String)` — os
  patterns de `case` `--policy | --min-node` e `--policy=* | --min-node=*` (o
  fish usa a lista de nomes).
- [ ] **Passo 1: Escrever os testes que falham**

```rust
// tests/init_lab (each shell: bash, zsh, sh, dash, ksh, fish)
#[test]
fn use_after_leading_policy_flags_switches_the_shell() {
    // eval "$(nvm init <shell>)"; nvm --min-node 18 use 20; node -v → v20.1.0, status 0
}

#[test]
fn use_after_a_joined_policy_flag_switches_the_shell() {
    // nvm --policy=/p.json use 20 (p.json allows 20) → node -v prints v20.1.0
}

#[test]
fn a_disallowed_use_leaves_the_shell_alone() {
    // nvm --min-node 22 use 20 → status 7; node -v still the previous version
}
```

- [ ] **Passo 2: Rodá-los** — esperado: falham (a função roteia pelo `$1`).

- [ ] **Passo 3: Implementar.** POSIX: antes do `case`, encontre o primeiro
  argumento que não é uma flag de política (pulando o valor depois de uma flag
  separada), guarde-o em `__nvmrc_command`, roteie por ele em vez de `${1-}` e
  faça unset de `__nvmrc_command` e das variáveis do loop nos dois ramos. Todos
  os argumentos continuam sendo passados ao binário sem mudanças, e ele mesmo
  remove as flags. Fish: o mesmo, com um loop `for` sobre `$argv` e uma
  variável local. Monte os patterns a partir de `POLICY_FLAGS` para que os
  planos seguintes só estendam a tabela.

- [ ] **Passo 4: Rodar** `cargo test` (incluindo o init lab) — esperado: passam
  em todo shell que o lab encontrar.

- [ ] **Passo 5: Commit** — `feat(shell): route the nvm function past leading
  policy flags`

### Tarefa 9: Documentação

**Arquivos:**

- Modificar: `README.md` (variáveis: `NVMRC_POLICY`; flags `--policy`,
  `--min-node`; a política do repositório; código de saída 9; modos legado e
  política), `docs/deviations.md` (as novas flags, `.nvmrc-policy.json`,
  aplicação em `use`/`exec`/`run` no modo política, saída 9, o roteamento da
  função de shell)

- [ ] **Passo 1: Escrever as seções** no estilo existente do README, apontando
  para `schemas/nvmrc-policy.v1.schema.json` e para a spec. Toda entrada de
  desvio cita o teste que a fixa (das Tarefas 4–8).

- [ ] **Passo 2: Rodar** `rumdl fmt` e depois `rumdl check README.md docs/` —
  esperado: nenhum problema.

- [ ] **Passo 3: Rodar o conjunto de pré-commit** — `cargo fmt --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
  `cargo deny check` — esperado: tudo limpo.

- [ ] **Passo 4: Commit** — `docs: document the security policy, its flags and
  exit code 9`
