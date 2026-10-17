# nvmrc Plano 12: Plano de implementação da checagem de CVE

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use
> superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para implementar este plano tarefa por tarefa. Os
> passos usam a sintaxe de checkbox (`- [ ]`) para acompanhamento.

**Objetivo:** Entregar a etapa 2 do design da política de segurança: checar a
versão do node contra o índice de vulnerabilidades do Node Security WG e falhar
na severidade igual ou acima do limite da política (padrão `high`), com
`--fail-on-severity` e `NVMRC_FAIL_ON_SEVERITY`, a regra `cve.source` da
política do repositório e os pendentes da revisão final do Plano 11.

**Arquitetura:** A parte pura fica em `src/domain/policy/`: `advisories.rs`
interpreta o índice e encontra os avisos que afetam uma versão numa plataforma;
`cve.rs` combina o limite de severidade e a localização do índice entre as
fontes. `commands/policy/mode.rs` devolve tanto as regras do node quanto as
configurações de CVE; `commands/policy/enforce.rs` busca o índice (cliente
rápido nos caminhos automáticos) e transforma os achados em uma violação ou em
um aviso. Os pontos de chamada do portão do Plano 11 não mudam.

**Stack:** Rust 2024 (MSRV 1.85), `serde_json` e `nodejs-semver` 7 já
existentes; nenhuma dependência nova.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.pt-BR.md`
(seções 6.1 `cve`, 6.3, 6.5, 6.6, 6.10; etapa de entrega 2). Parte de
`feat/policy-cve` em `8ab9999` (Plano 11 integrado).

**Como é o índice real** (verificado em 2026-10-17, 194 entradas): um objeto
indexado por id (`"192"`), cada entrada com `cve` (array, vazio em 14 entradas),
`vulnerable` e `patched` (intervalos npm; `vulnerable` nomeia linhas inteiras
como `22.x || 24.x`, `patched` nomeia a correção como `^22.23.2 || ^24.18.1`;
alguns intervalos trazem espaços iniciais, um é `<= 10`), `affectedEnvironments`
(`all`, `linux`, `darwin`, `win32`, `aix`, `freebsd`, `openbsd`, `sunos`,
`android`), `severity` (`critical`, `high`, `medium`, `low` e `unknown` em 13
entradas), além de campos que este plano ignora (`description`, `overview`,
`ref`, `author`, `cvss`, ...).

## Restrições globais

- Uma versão é afetada por um aviso quando satisfaz `vulnerable` e **não**
  satisfaz `patched`.
- Um aviso conta quando seu `affectedEnvironments` contém `all` ou o slug do
  SO da plataforma atual (`Os::slug()`); sem plataforma conhecida, ele conta.
- Ordem de severidade: `low` < `medium` < `high` < `critical`. Um achado falha
  quando seu posto é igual ou acima do limite; o limite `none` não falha nada,
  mas ainda reporta. Limite padrão `high`.
- Uma `severity` ausente e o `unknown` do índice contam como `high`; qualquer
  outro valor, um intervalo que não se interpreta ou um índice que não é um
  objeto de entradas é uma falha em saber (saída 9).
- Fontes do limite de severidade: `--fail-on-severity`,
  `NVMRC_FAIL_ON_SEVERITY`, `cve.failOnSeverity` central, `cve.failOnSeverity`
  do repositório; vence o mais estrito; um override mais frouxo é um aviso; um
  empate cita a política central.
- Localização do índice: `cve.source` central quando há política central
  configurada (senão o padrão, mesmo que o repositório nomeie um); `cve.source`
  do repositório só sem política central; padrão
  `https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json`.
- O índice é lido como uma política (`read_location`): só https, nunca
  `http://`; cliente rápido nos caminhos automáticos.
- Comandos explícitos: um achado igual ou acima do limite sai com 7; uma falha
  em saber sai com 9. Caminhos automáticos avisam e passam. Achados abaixo do
  limite imprimem um aviso e nunca mudam o status.
- Sem configuração de política → comportamento exatamente o de hoje; o modo
  legado nunca busca o índice.
- Arquivos com menos de 300 linhas, funções com menos de 30, complexidade
  abaixo de 10, sem unwrap/expect em entrada externa, `cargo fmt`, `cargo clippy
  --all-targets --all-features -- -D warnings`, `rumdl` no Markdown alterado,
  sem rede nos testes, TDD; commits Conventional, assinados com GPG, sem
  atribuição de IA.

## Foco da revisão

- Um aviso com array `cve` vazio: nomeado `advisory <id>` nas mensagens, nunca
  um marcador vazio (teste da Tarefa 2
  `an_advisory_without_cve_ids_is_named_by_id`).
- Uma versão numa linha corrigida (`v22.23.2` sob `22.x` / `^22.23.2`): não
  afetada (teste da Tarefa 2 `a_patched_version_is_not_affected`).
- `--fail-on-severity HIGH` ou `critial`: saída 9 nomeando a flag, não uma
  aprovação com o padrão (teste da Tarefa 4
  `an_invalid_severity_flag_is_a_failure_to_know`).
- Um `cve.source` relativo numa política do repositório, executado a partir de
  um subdiretório: resolvido a partir do diretório do arquivo da política
  (teste da Tarefa 4 `a_relative_index_location_is_read_next_to_its_policy`).
- `install --offline` com o índice padrão: saída 9, nenhuma requisição de rede
  (teste da Tarefa 5 `offline_with_a_remote_index_is_a_failure_to_know`).

---

### Tarefa 1: Pendentes que não tocam o código de política

**Arquivos:**

- Modificar: `Cargo.lock` (rustls), `README.md:251` (redação das flags de
  exec/run)
- [ ] **Passo 1: Atualizar o rustls** — `cargo update -p rustls` (0.23.45 →
  0.23.46 resolve para o MSRV 1.85). Rode `cargo deny check advisories`. Se
  RUSTSEC-2026-0346/0347/0348 permanecerem, rode `cargo update -p rustls
  --precise <primeira versão corrigida>` conforme a linha "Solution" dos
  avisos, ficando numa versão cujo `rust-version` seja ≤ 1.85; se não existir,
  reverta a mudança no lockfile e registre isso no relatório.
- [ ] **Passo 2: Corrigir a redação do README** — depois de `exec` e `run`, as
  flags de política são tomadas só imediatamente após o subcomando (qualquer
  outra opção no meio as encerra), de acordo com `docs/deviations.md`.
- [ ] **Passo 3: Verificar** — `cargo test`, `cargo deny check`, `rumdl check
  README.md` — esperado: testes passam; deny limpo ou o resíduo registrado.
- [ ] **Passo 4: Commit** — `fix(deps): update rustls for RUSTSEC-2026-0346
  to 0348` e `docs: say exactly where exec and run take policy flags`.

### Tarefa 2: Interpretando o índice de vulnerabilidades

**Arquivos:**

- Criar: `src/domain/policy/advisories.rs`,
  `src/domain/policy/advisories_tests.rs`
- Modificar: `src/domain/policy/mod.rs` (módulo, reexportações, módulo de
  testes), `src/domain/policy/document.rs` (`Severity::rank`)

**Interfaces:**

- Produz:
  - `impl Severity { pub fn rank(self) -> u8 }` — `None` 0, `Low` 1, `Medium`
    2, `High` 3, `Critical` 4
  - `struct Advisory { pub id: String, pub cves: Vec<String>, pub vulnerable:
    nodejs_semver::Range, pub patched: Option<nodejs_semver::Range>, pub
    severity: Severity, pub environments: Vec<String> }`
  - `enum IndexError { NotAnIndex(String), BadEntry { id: String, reason:
    String } }` (thiserror, PartialEq)
  - `pub fn parse_advisories(text: &str) -> Result<Vec<Advisory>, IndexError>`
  - `pub struct Finding { pub name: String, pub severity: Severity }` — `name`
    é o id do CVE, ou `advisory <id>` quando `cves` está vazio; um achado por
    id de CVE
  - `pub fn findings(advisories: &[Advisory], version: &Version, os:
    Option<&str>) -> Vec<Finding>`

- [ ] **Passo 1: Escrever os testes que falham** (`advisories_tests.rs`), com
  uma fixture inline copiando entradas reais: `"192"` (`22.x || 24.x || 26.x` /
  `^22.23.2 || ^24.18.1 || ^26.5.1`, high), uma entrada só de `win32`, uma com
  `"cve": []`, uma com `"severity": "unknown"`, uma com um intervalo com espaço
  inicial (`" 10.x || 12.x"`), uma `"<= 10"`.

```rust
#[test]
fn a_vulnerable_unpatched_version_is_affected() {
    let found = findings(&fixture(), &v("v22.20.0"), Some("linux"));
    assert!(found.iter().any(|f| f.name == "CVE-2026-58043" && f.severity == Severity::High));
}

#[test]
fn a_patched_version_is_not_affected() {
    assert!(findings(&fixture(), &v("v22.23.2"), Some("linux")).iter().all(|f| f.name != "CVE-2026-58043"));
}

#[test]
fn a_platform_specific_advisory_counts_only_on_its_platform() {
    assert!(findings(&fixture(), &v(WIN_VERSION), Some("linux")).iter().all(|f| f.name != WIN_CVE));
    assert!(findings(&fixture(), &v(WIN_VERSION), None).iter().any(|f| f.name == WIN_CVE));
}

#[test]
fn an_advisory_without_cve_ids_is_named_by_id() {
    assert!(findings(&fixture(), &v(NO_CVE_VERSION), Some("linux")).iter().any(|f| f.name == "advisory 12"));
}

#[test]
fn unknown_and_missing_severities_count_as_high() {
    let unknown = parse_advisories(&entry_with(r#""severity": "unknown""#)).unwrap();
    let missing = parse_advisories(&entry_without_severity()).unwrap();
    assert_eq!((unknown[0].severity, missing[0].severity), (Severity::High, Severity::High));
}

#[test]
fn anything_else_is_a_failure_to_know() {
    assert!(matches!(parse_advisories("[]"), Err(IndexError::NotAnIndex(_))));
    assert!(matches!(parse_advisories(&entry_with(r#""severity": "urgent""#)), Err(IndexError::BadEntry { .. })));
    assert!(matches!(parse_advisories(&entry_with(r#""vulnerable": ">>22""#)), Err(IndexError::BadEntry { .. })));
}

#[test]
fn ranges_with_spaces_and_comparators_parse() {
    assert!(parse_advisories(FIXTURE).is_ok());
}

#[test]
fn severities_rank_from_none_to_critical() {
    assert!(Severity::None.rank() < Severity::Low.rank() && Severity::High.rank() < Severity::Critical.rank());
}
```

- [ ] **Passo 2: Rodá-los** — `cargo test domain::policy::advisories` —
  esperado: falha de compilação.
- [ ] **Passo 3: Implementar.** Desserializar em `BTreeMap<String, RawEntry>`
  (campos desconhecidos permitidos — o índice não é nosso); aparar os
  intervalos antes de `Range::parse`; `patched` vazio ou ausente é `None`;
  versões io.js nunca casam (os chamadores as pulam, como nas outras regras).
  Comparar por meio de `nodejs_semver::Version` montada a partir de
  `major.minor.patch`.
- [ ] **Passo 4: Rodar** `cargo test domain::policy` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(policy): parse the vulnerability index and
  find what affects a version`

### Tarefa 3: Combinando as configurações de CVE

**Arquivos:**

- Criar: `src/domain/policy/cve.rs`, `src/domain/policy/cve_tests.rs`
- Modificar: `src/domain/policy/source.rs` (`Source::Default`),
  `src/domain/policy/mod.rs`

**Interfaces:**

- Consome: `Severity` (Tarefa 2), `Source`, `Sourced`.
- Produz:
  - `Source::Default` — Display `the default`; `is_override()` falso
  - `pub const DEFAULT_INDEX: &str =
    "https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json";`
  - `struct CveLayer { pub source: Source, pub limit: Option<Severity>, pub
    index: Option<String> }`
  - `struct CvePolicy { pub limit: Sourced<Severity>, pub index:
    Sourced<String> }`
  - `pub fn merge_cve(layers: Vec<CveLayer>) -> (CvePolicy, Vec<String>)`
- [ ] **Passo 1: Escrever os testes que falham**

```rust
#[test]
fn without_any_setting_the_limit_is_high_and_the_index_is_the_default() {
    let (policy, warnings) = merge_cve(vec![]);
    assert_eq!(policy.limit, Sourced { value: Severity::High, source: Source::Default });
    assert_eq!(policy.index.value, DEFAULT_INDEX);
    assert!(warnings.is_empty());
}

#[test]
fn the_strictest_limit_wins_and_a_looser_override_warns() {
    let (policy, warnings) = merge_cve(vec![central(Some(Severity::High), None), flag(Some(Severity::Critical))]);
    assert_eq!(policy.limit.value, Severity::High);
    assert_eq!(warnings, vec!["nvm: --fail-on-severity critical is looser than high from policy /p.json; using high."]);
    let (policy, _) = merge_cve(vec![central(Some(Severity::High), None), variable(Some(Severity::Low))]);
    assert_eq!(policy.limit, Sourced { value: Severity::Low, source: Source::Variable("NVMRC_FAIL_ON_SEVERITY") });
}

#[test]
fn none_is_the_loosest_limit() {
    let (policy, warnings) = merge_cve(vec![central(Some(Severity::Medium), None), repository(Some(Severity::None), None)]);
    assert_eq!(policy.limit.value, Severity::Medium);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_central_index_wins_and_the_repository_index_counts_only_alone() {
    let (with_central, _) = merge_cve(vec![central(None, None), repository(None, Some("/w/vuln.json"))]);
    assert_eq!(with_central.index.value, DEFAULT_INDEX);
    let (alone, _) = merge_cve(vec![repository(None, Some("/w/vuln.json"))]);
    assert_eq!(alone.index, Sourced { value: "/w/vuln.json".into(), source: Source::RepositoryPolicy("/w/.nvmrc-policy.json".into()) });
}
```

- [ ] **Passo 2: Rodá-los** — esperado: falha de compilação.
- [ ] **Passo 3: Implementar.** Mais estrito = maior `rank()`, exceto que
  `None` (posto 0) é mais frouxo que todos; empates vão para a primeira camada
  (os chamadores passam a camada central primeiro). Texto do aviso, exato:
  `nvm: {source} {value} is looser than {winner} from {winner_source}; using
  {winner}.` (valores em minúsculas, como no JSON). O índice: o `index` da
  camada central quando há uma camada central (senão `DEFAULT_INDEX` com
  `Source::Default`), o da camada do repositório só quando não há camada
  central.
- [ ] **Passo 4: Rodar** `cargo test domain::policy` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(policy): merge the severity limit and the
  index location`

### Tarefa 4: A flag de severidade e o modo

**Arquivos:**

- Modificar: `src/domain/policy/flags.rs` (tabela), `src/commands/policy/load.rs`
  (modo política em qualquer variável de política `NVMRC_`),
  `src/commands/policy/mode.rs` (`Mode::Enforced` carrega `EnforcedPolicy`),
  `src/commands/policy/enforce.rs` (adaptar só ao novo formato do modo)
- Teste: `src/commands/policy/tests/mode.rs`,
  `src/domain/policy/flags_tests.rs`, testes dos trechos de `src/shell/init/`

**Interfaces:**

- Consome: `CveLayer`, `CvePolicy`, `merge_cve` (Tarefa 3), `parse_document`.
- Produz:
  - `POLICY_FLAGS` ganha `{ name: "--fail-on-severity", variable:
    "NVMRC_FAIL_ON_SEVERITY" }` (os padrões dos shells acompanham
    automaticamente)
  - `pub struct EnforcedPolicy { pub node: NodePolicy, pub cve: CvePolicy }`
  - `pub enum Mode { Legacy, Enforced(EnforcedPolicy) }`
  - assinatura de `resolve_mode` inalterada
- [ ] **Passo 1: Escrever os testes que falham**

```rust
#[test]
fn the_severity_flag_and_variable_turn_policy_mode_on() {
    assert!(policy_mode_requested(&ctx_with_var("NVMRC_FAIL_ON_SEVERITY", "low")));
    assert!(policy_mode_requested(&ctx_with_flag("--fail-on-severity", "low")));
}

#[test]
fn an_invalid_severity_flag_is_a_failure_to_know() {
    let error = resolve_mode(&ctx_with_flag("--fail-on-severity", "critial")).unwrap_err();
    assert_eq!(error, PolicyError::Invalid {
        location: "--fail-on-severity".into(),
        reason: "'critial' is not one of critical, high, medium, low, none".into(),
    });
}

#[test]
fn the_cve_settings_of_both_policies_take_part() {
    // central {cve: {failOnSeverity: medium}}, repository {cve: {failOnSeverity: low, source: "v.json"}}
    let (Mode::Enforced(policy), _) = resolve_mode(&both_policies_ctx()).unwrap() else { panic!() };
    assert_eq!(policy.cve.limit.value, Severity::Low);
    assert_eq!(policy.cve.index.value, DEFAULT_INDEX);
}

#[test]
fn a_relative_index_location_is_read_next_to_its_policy() {
    // /w/.nvmrc-policy.json {cve: {source: "sec/vuln.json"}}, PWD=/w/app/src
    let (Mode::Enforced(policy), _) = resolve_mode(&repo_ctx_with_relative_index()).unwrap() else { panic!() };
    assert_eq!(policy.cve.index.value, "/w/sec/vuln.json");
}

#[test]
fn a_relative_index_in_a_remote_policy_is_invalid() {
    // NVMRC_POLICY=https://sec/p.json with cve.source "vuln.json"
    assert!(matches!(resolve_mode(&remote_ctx_with_relative_index()).unwrap_err(), PolicyError::Invalid { .. }));
}
```

Mais o teste de flag `the_severity_flag_is_taken_like_the_others` (antes e
depois do subcomando, forma com `=`) e o teste de trecho de que os padrões do
POSIX e do fish incluem `--fail-on-severity`.

- [ ] **Passo 2: Rodá-los** — esperado: falhas.
- [ ] **Passo 3: Implementar.** `policy_mode_requested` passa a incluir:
  qualquer variável de `POLICY_FLAGS` que comece com `NVMRC_` e esteja definida
  e não em branco. `resolve_mode` monta as camadas de CVE na ordem central,
  repositório, flag, variável (central primeiro, como no node). Um `cve.source`
  relativo numa política do repositório vira `<diretório da política>/<source>`;
  numa política central lida de um caminho, `<diretório desse arquivo>/<source>`;
  numa política central lida de https, é `PolicyError::Invalid` nomeando a
  localização da política. Um valor de severidade inválido vindo da flag ou da
  variável é `PolicyError::Invalid` com o motivo exato acima. Um documento sem
  `tools.node` ainda contribui com sua camada de CVE.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(policy): add --fail-on-severity and carry
  the CVE settings in the mode`

### Tarefa 5: Aplicando a checagem de CVE

**Arquivos:**

- Modificar: `src/commands/policy/enforce.rs` (busca, achados, relatório),
  `src/domain/policy/verdict.rs` (`Violation::Vulnerable`)
- Criar: `src/commands/policy/advisories.rs` (buscar e julgar, para manter
  `enforce.rs` abaixo de 300 linhas)
- Teste: `src/commands/policy/tests/advisories.rs`,
  `src/commands/install/tests/policy.rs`, `src/commands/use_version/tests*`

**Interfaces:**

- Consome: `parse_advisories`, `findings`, `Finding` (Tarefa 2), `CvePolicy`
  (Tarefa 3), `EnforcedPolicy` (Tarefa 4), `read_location`,
  `Context::platform()`.
- Produz:
  - `Violation::Vulnerable { findings: Vec<Finding>, limit: Sourced<Severity> }`
    — Display `CVE-2024-27983 (high), CVE-2024-22019 (high); the limit is high
    (from policy /p.json)`
  - `pub(super) fn judge(context: &Context<'_>, requested: &Requested, cve:
    &CvePolicy) -> Result<Judgement, PolicyError>` com `struct Judgement { pub
    failing: Vec<Finding>, pub below: Vec<Finding> }`
  - `PolicyError::IndexInvalid { location: String, reason: String }` —
    mensagem `nvm: cannot check the policy: the vulnerability index {location}
    is not valid: {reason}`
- [ ] **Passo 1: Escrever os testes que falham**

```rust
#[test]
fn a_finding_at_the_limit_stops_with_7() {
    // --min-node unset, NVMRC_FAIL_ON_SEVERITY=high, FakeHttp serves the fixture at DEFAULT_INDEX
    let (gate, transcript) = gate(&ctx_with_index(), "v22.20.0");
    assert!(matches!(gate, Gate::Stop(NvmExitCode::BelowVersionFloor)));
    assert!(stderr(transcript).contains("  - CVE-2026-58043 (high)"));
    assert!(stderr(transcript).contains("the limit is high (from NVMRC_FAIL_ON_SEVERITY)"));
}

#[test]
fn findings_below_the_limit_only_warn() {
    let (gate, transcript) = gate(&ctx_with_limit("critical"), "v22.20.0");
    assert!(matches!(gate, Gate::Pass));
    assert!(stderr(transcript).starts_with("nvm: warning: node v22.20.0 has known vulnerabilities below the limit critical:"));
}

#[test]
fn limit_none_reports_and_passes() {
    let (gate, transcript) = gate(&ctx_with_limit("none"), "v22.20.0");
    assert!(matches!(gate, Gate::Pass));
    assert!(stderr(transcript).contains("CVE-2026-58043 (high)"));
}

#[test]
fn an_unreachable_index_is_a_failure_to_know() {
    let error = enforce_node(&ctx_without_index(), &requested("v22.20.0"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}

#[test]
fn an_invalid_index_names_its_location() {
    let error = enforce_node(&ctx_with_body("[]"), &requested("v22.20.0"), &mut Transcript::default()).unwrap_err();
    assert!(error.to_string().contains("the vulnerability index https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json is not valid"));
}

#[test]
fn offline_with_a_remote_index_is_a_failure_to_know() {
    let error = enforce_node(&ctx_with_index(), &offline("v22.20.0"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
    assert!(http_requests().is_empty());
}

#[test]
fn offline_with_a_local_index_is_checked() { /* repository policy cve.source = local file → judged, no request */ }

#[test]
fn an_automatic_path_warns_on_a_finding_and_passes() { /* .advisory() context → Pass, warning header */ }

#[test]
fn legacy_mode_never_fetches_the_index() { /* NVM_MIN_VERSION only → no request */ }

#[test]
fn iojs_versions_skip_the_cve_check() { /* NotNode only, no index request */ }
```

Em `install/tests/policy.rs`:
`install_of_a_vulnerable_version_stops_before_downloading`. Nos testes de
`use`: `use_of_a_vulnerable_installed_version_stops_without_switching`.

- [ ] **Passo 2: Rodá-los** — esperado: falhas.
- [ ] **Passo 3: Implementar.** Em `check`, depois das violações do node: pular
  a checagem de CVE para io.js; caso contrário, `judge`. `judge` lê o índice
  com `read_location(context, &cve.index.value)` (offline e um índice `https://`
  → `ReleaseInfoUnavailable("offline")` antes de qualquer requisição; reusar a
  variante existente), interpreta-o (`IndexError` → `PolicyError::IndexInvalid`),
  toma `context.platform().map(|p| p.os.slug())` e divide `findings` por
  `severity.rank() >= limit.rank()` (limite `None`: tudo é `below`). `failing`
  não vazio → uma `Violation::Vulnerable` acrescentada às violações do node
  (mesmo cabeçalho e mesma linha de correção). `below` não vazio → uma linha de
  aviso `nvm: warning: node {version} has known vulnerabilities below the limit
  {limit}: {names with severities}` impressa seja qual for a decisão do portão.
  A busca acontece uma vez por comando.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa, suíte de
  compatibilidade incluída.
- [ ] **Passo 5: Commit** — `feat(policy): fail on known vulnerabilities at or
  above the severity limit`

### Tarefa 6: `use --save` passa pelo portão antes de salvar

**Arquivos:**

- Modificar: `src/commands/use_version/mod.rs`,
  `src/commands/use_version/target/mod.rs` (só se necessário)
- Teste: `src/commands/use_version/tests*`

**Interfaces:**

- Consome: `policy_mode_requested`, o portão do Plano 11
  (`commands::policy::gate`).

- [ ] **Passo 1: Escrever os testes que falham**

```rust
#[test]
fn a_refused_use_save_leaves_nvmrc_untouched() {
    // installed v20.1.0, --min-node 22, /w/.nvmrc contains "v22.11.0"
    let output = run(&ctx, &args(&["--save", "20"])).unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(fs.read_to_string(Path::new("/w/.nvmrc")).unwrap(), "v22.11.0\n");
}

#[test]
fn an_allowed_use_save_still_writes_nvmrc() { /* --min-node 18 → .nvmrc becomes v20.1.0 */ }

#[test]
fn legacy_use_save_keeps_its_order() { /* no policy: today's output byte for byte */ }
```

- [ ] **Passo 2: Rodá-los** — esperado: o primeiro falha.
- [ ] **Passo 3: Implementar.** Só no modo política, resolver o alvo com o
  salvamento adiado, rodar o portão e escrever o `.nvmrc` apenas quando ele
  passar; o modo legado mantém a ordem do nvm.sh (salvar antes das checagens).
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `fix(policy): write .nvmrc only after the policy
  allows the version`

### Tarefa 7: Documentação

**Arquivos:**

- Modificar: `README.md` (checagem de CVE, `--fail-on-severity`,
  `NVMRC_FAIL_ON_SEVERITY`, o índice e seu espelho, offline, `use --save`),
  `docs/deviations.md` (cada entrada com seu teste que a fixa)
- [ ] **Passo 1: Escrever as seções** no estilo existente; declarar a regra
  vulnerável-e-não-corrigido, `unknown` = `high`, a filtragem por plataforma, a
  URL padrão do índice e a regra de `cve.source`. Remover a redação "CVE:
  aceito mas não aplicado".
- [ ] **Passo 2: Rodar** `rumdl fmt` e depois `rumdl check README.md docs/`,
  então `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
  warnings`, `cargo test`, `cargo deny check` — esperado: limpo.
- [ ] **Passo 3: Commit** — `docs: document the CVE check and
  --fail-on-severity`
