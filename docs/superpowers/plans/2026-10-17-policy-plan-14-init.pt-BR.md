# nvmrc Plano 14: Plano de implementação do modo de projeto do `nvm init`

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use
> superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para implementar este plano tarefa por tarefa. Os
> passos usam a sintaxe de checkbox (`- [ ]`) para acompanhamento.

**Objetivo:** Entregar a etapa 4 do design da política de segurança: `nvm init
--node[=<v>] --npm[=<v>] --yarn[=<v>] --pnpm[=<v>]` escreve o `.nvmrc` de um
projeto (e `packageManager` num `package.json` existente) com versões que já
satisfazem a política — a versão permitida mais nova quando nenhuma versão é
dada, ou uma versão exata verificada.

**Arquitetura:** O `nvm init` mantém o modo shell e ganha um modo de projeto
escolhido por suas flags. A parte pura edita os arquivos
(`domain/nvmrc/edit.rs`, `domain/package_json.rs`) e lista as versões do
registro (`domain/registry.rs`). Um único `Judge` (`commands/policy/judge.rs`)
carrega a política e o índice de vulnerabilidades uma só vez e decide se uma
versão de uma ferramenta é permitida, reaproveitando `ToolPolicy::violations` e
a separação por CVE que a aplicação das regras já usa. `commands/init/` resolve
cada ferramenta pedida, recusa antes de escrever qualquer coisa e só então
escreve.

**Stack:** Rust 2024 (MSRV 1.85); `serde_json` e `nodejs-semver` já
existentes; nenhuma dependência nova.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.pt-BR.md`
(seções 6.2, 6.5, 6.7, 6.8, 6.10; etapa de entrega 4). Parte de
`feat/policy-init` em `faabb1c` (Planos 11–13 integrados).

## Restrições globais

- O modo shell não muda: `nvm init <shell> [--no-use|--install]` imprime o
  mesmo trecho; `nvm init` sem shell e sem flag de projeto continua imprimindo
  o uso com status 127.
- Modo de projeto: `nvm init` com uma ou mais de `--node[=<v>]`,
  `--npm[=<v>]`, `--yarn[=<v>]`, `--pnpm[=<v>]` e sem nome de shell. Um nome de
  shell com uma flag de projeto, `--yarn` com `--pnpm`, ou uma flag dada duas
  vezes é um erro de uso, saída 6 (`InvalidOptions`).
- Sem versão, a ferramenta recebe a versão permitida mais nova (spec 6.2): node,
  a versão **LTS** permitida mais nova (com ou sem `ltsOnly`); npm, yarn, pnpm,
  o `major.minor.patch` estável permitido mais novo no registro (nunca
  pré-lançamentos). Os candidatos do yarn vêm de `@yarnpkg/cli-dist`.
- Com versão, ela deve ser exata (node: `20.11.1` ou `v20.11.1`; npm, yarn,
  pnpm: `major.minor.patch` só com dígitos, como nas fixações), publicada (no
  índice do node, ou no pacote do registro para essa versão — yarn abaixo de
  2.0.0 é `yarn`) e permitida; caso contrário o comando falha antes de
  escrever.
- "Permitida" é a spec 6.2 por inteiro, decidida por uma única função: mínimo,
  allow, deny, `ltsOnly` (node), `minimumReleaseAgeDays` e, para o node,
  nenhum CVE igual ou acima do limite. Sem política (modo legado) toda versão
  publicada é permitida; o piso legado vale só para `install` (spec 6.5).
- Códigos de saída: uma versão que não é exata ou não foi publicada, ou um
  conflito de fixação, sai com 3; não permitida, ou nenhuma versão permitida,
  sai com 7; uma falha em saber (índice, registro ou política ilegível) sai
  com 9; erros de uso, 6.
- Arquivos: `.nvmrc` no **diretório de trabalho** (criado quando ausente): só
  as chaves pedidas mudam; a linha de versão solta para o node, linhas
  `npm=`/`yarn=`/`pnpm=` para os demais; toda outra linha, comentário e ordem
  são mantidos. `packageManager` é definido no `package.json` do diretório de
  trabalho somente quando ele existe e yarn ou pnpm foi pedido; nada mais nele
  muda.
- Os dois novos conteúdos são calculados e validados antes de qualquer arquivo
  ser escrito. Pedir yarn quando o `.nvmrc` já fixa pnpm (ou o inverso) é
  recusado (saída 3), nunca trocado em silêncio.
- Rede: o índice do node por `context.http()`, o registro por
  `context.third_party_http()` (regras de credenciais do Plano 13); o packument
  é buscado como bytes.
- Sem fixações e sem política em outro lugar: comportamento exatamente como
  hoje.
- Arquivos com menos de 300 linhas, funções com menos de 30 (da assinatura à
  chave de fechamento), complexidade abaixo de 10, nenhum unwrap/expect em
  entrada externa, `cargo fmt`, `cargo clippy --all-targets --all-features --
  -D warnings`, `rumdl` no Markdown alterado, sem rede nos testes, TDD;
  commits Conventional, assinados com GPG (`git commit -S`), sem atribuição de
  IA.

## Foco da revisão

- Um `package.json` cujo objeto aninhado também tem uma chave
  `packageManager` (por exemplo sob `"devEngines"`): só o campo de nível
  superior é definido (teste da Tarefa 2
  `only_the_top_level_package_manager_changes`).
- Um `.nvmrc` existente com comentários, outras chaves e fins de linha CRLF:
  só as linhas pedidas mudam, os fins de linha são mantidos (teste da Tarefa 2
  `editing_keeps_comments_keys_and_line_endings`).
- Uma política que nega o LTS mais novo e exige uma idade de lançamento: o
  próximo LTS permitido é escolhido, não o mais novo (teste da Tarefa 4
  `the_newest_allowed_lts_skips_denied_and_too_recent_releases`).
- Pré-lançamentos no packument (`10.0.0-rc.1`) ou um `dist-tags.latest` que não
  é permitido: nunca escolhidos (teste da Tarefa 4
  `prereleases_and_dist_tags_are_ignored`).
- `--node=20`, `--node=iojs-v3.3.1`, `--npm=v10.9.0`, `--pnpm=latest`: saída 3
  antes de qualquer requisição (teste da Tarefa 1
  `inexact_versions_are_rejected`).

---

### Tarefa 1: Argumentos do modo de projeto

**Arquivos:**

- Criar: `src/commands/init/mod.rs` (a partir de `src/commands/init.rs`; modo
  shell movido sem alterações), `src/commands/init/request.rs` (com testes)
- Modificar: `src/commands/mod.rs`, `src/cli/commands.rs` (passar o contexto
  para `init::run`), `src/shell/init/mod.rs` (`USAGE`)

**Interfaces:**

- Produz:
  - `enum Wanted { Newest, Exact(Version) }`
  - `struct ProjectRequest { pub node: Option<Wanted>, pub npm:
    Option<Wanted>, pub yarn: Option<Wanted>, pub pnpm: Option<Wanted> }`
    com `fn get(&self, tool: Tool) -> Option<&Wanted>`
  - `enum InitMode { Shell(Shell, InitOptions), Project(ProjectRequest) }`
  - `fn parse(args: &[String]) -> Result<InitMode, CliError>`: erros do modo
    shell como hoje; `CliError::InvalidOptions` (saída 6) para um nome de shell
    com uma flag de projeto (`nvm init: a shell name cannot be combined with
    --node, --npm, --yarn or --pnpm.`), `--yarn` com `--pnpm` (`nvm init:
    --yarn and --pnpm cannot be used together.`), uma flag dada duas vezes
    (`nvm init: --npm is given twice.`); e um novo
    `CliError::InvalidVersionArgument(String)`, mapeado para
    `NvmExitCode::InvalidVersion` (saída 3), para uma versão inexata (`nvm
    init: --npm=10 is not an exact version (major.minor.patch).`)
  - `pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output,
    CliError>`: modo shell como hoje; o modo de projeto devolve
    `CliError::Unsupported("nvm init: project mode is not available yet.")`
    até a Tarefa 5
  - `USAGE` passa a ter duas linhas: `Usage: nvm init <shell> [--no-use]
    [--install]` e `nvm init [--node[=<version>]]
    [--npm[=<version>]] [--yarn[=<version>] | --pnpm[=<version>]]`
- [ ] **Passo 1: Escrever os testes que falham** — `request.rs`:
  `a_shell_name_is_shell_mode` (`["bash","--no-use"]`),
  `project_flags_without_a_version_want_the_newest`,
  `project_flags_with_a_version_want_it_exactly` (`--node=v20.11.1`,
  `--pnpm=9.12.0`), `a_shell_with_a_project_flag_is_a_usage_error` (saída 6),
  `yarn_and_pnpm_together_are_a_usage_error`,
  `a_repeated_flag_is_a_usage_error`, `inexact_versions_are_rejected`
  (`--node=20`, `--node=iojs-v3.3.1`, `--npm=v10.9.0`, `--pnpm=latest` →
  saída 3 com a mensagem). Manter verdes os testes existentes da CLI do init
  (`src/cli/tests/init.rs`).
- [ ] **Passo 2: Rodar** `cargo test init` — esperado: falha.
- [ ] **Passo 3: Implementar** a divisão do módulo, `parse`, a nova variante de
  erro e a mudança no despacho.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa; trechos de shell
  idênticos byte a byte.
- [ ] **Passo 5: Commit** — `feat(init): parse the project-mode flags`.

### Tarefa 2: Edição do `.nvmrc` e do `package.json`

**Arquivos:**

- Criar: `src/domain/nvmrc/edit.rs` (+ `edit_tests.rs`); mover `with_version`
  de `src/domain/nvmrc/mod.rs` para lá (reexportando-o)
- Modificar: `src/domain/nvmrc/mod.rs`, `src/domain/package_json.rs` (+ testes)

**Interfaces:**

- Consome: `Tool` (`src/domain/tool.rs`), `package_manager_field`.
- Produz:
  - `fn with_pin(contents: &str, tool: Tool, version: &Version) -> String`:
    substitui a linha `<tool>=` existente (mesmas regras de trim e comentário de
    `parse_pins`, um comentário final é mantido) ou acrescenta
    `<tool>=<x.y.z>` depois da última linha não vazia; `with_version` e
    `with_pin` mantêm o fim de linha de cada linha (`\n` ou `\r\n`) e a nova
    linha final do arquivo
  - `fn set_package_manager(text: &str, value: &str) -> Result<String,
    PackageManagerError>`: o valor string de `"packageManager"` do objeto de
    nível superior substituído no lugar, ou o membro acrescentado como o último
    membro de nível superior usando a indentação do primeiro membro (`"\n  "`
    quando o objeto está vazio, `{}` vira `{\n  "packageManager": "<v>"\n}`);
    todo o resto byte a byte; `Malformed` quando `text` não é um objeto JSON
    (validar com `serde_json` e depois editar varrendo tokens na profundidade 1
    — strings e escapes respeitados)
- [ ] **Passo 1: Escrever os testes que falham** — `edit_tests.rs`:
  `with_pin_replaces_the_existing_line` (`" pnpm = 9.1.0 # old"` →
  `"pnpm=9.12.0 # old"`), `with_pin_appends_a_new_line`,
  `editing_keeps_comments_keys_and_line_endings` (um arquivo CRLF com um
  comentário, uma linha solta sem `node`, `npm=` e uma chave desconhecida:
  depois de `with_version` e `with_pin` só essas duas linhas diferem, CRLF
  mantido), `with_version_on_an_empty_file_is_just_the_version`.
  `package_json.rs`: `replaces_the_package_manager_value`,
  `appends_the_field_with_the_file_indentation` (arquivos com 2 espaços e com
  tab), `an_empty_object_gets_the_field`,
  `only_the_top_level_package_manager_changes` (um
  `"devEngines": {"packageManager": "x"}` aninhado permanece),
  `escaped_quotes_in_strings_do_not_confuse_the_scan`,
  `a_top_level_array_is_malformed`.
- [ ] **Passo 2: Rodar** `cargo test nvmrc package_json` — esperado: falha.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa (os testes de `--save`
  do Plano 13 continuam verdes; isso também corrige o problema menor de fins de
  linha mistos com CRLF).
- [ ] **Passo 5: Commit** — `feat(nvmrc): edit pins and packageManager in
  place`.

### Tarefa 3: Um único juiz para "permitida"

**Arquivos:**

- Criar: `src/commands/policy/judge.rs`, `src/commands/policy/tests/judge.rs`
- Modificar: `src/commands/policy/advisories.rs` (compartilhar o carregamento
  do índice e a separação por severidade), `src/commands/policy/mod.rs`

**Interfaces:**

- Consome: `resolve_mode`, `EnforcedPolicy::tool`, `ToolPolicy::violations`,
  `CvePolicy`, `parse_advisories`, `findings`, `read_location_with`.
- Produz:
  - `pub(super) fn load_advisories(context: &Context<'_>, cve: &CvePolicy) ->
    Result<Vec<Advisory>, PolicyError>` e
    `pub(super) fn failing(findings: Vec<Finding>, limit: Severity) ->
    Judgement` (o `split` de hoje), usados por `judge` e por `Judge`
  - `pub struct Judge` com `pub fn load(context: &Context<'_>, transcript:
    &mut Transcript) -> Result<Judge, PolicyError>` (resolve o modo uma só vez,
    imprime seus avisos; modo legado → um juiz que permite tudo) e
    `pub fn violations(&mut self, context: &Context<'_>, tool: Tool, version:
    &Version, release: &ReleaseInfo) -> Result<Vec<Violation>, PolicyError>`
    (as regras da ferramenta e, para o node, a checagem de CVE; o índice é lido
    na primeira chamada de node e reaproveitado)
  - `ReleaseInfo` é o tipo pelo qual os dados do lançamento são passados (a
    Tarefa 4 o fornece para cada candidato), então `MissingReleaseInfo` não
    pode ocorrer aqui
- [ ] **Passo 1: Escrever os testes que falham** — `tests/judge.rs`:
  `legacy_mode_allows_everything` (sem política, sem requisição ao índice),
  `a_denied_version_is_not_allowed`,
  `a_node_cve_above_the_limit_is_not_allowed`,
  `the_index_is_read_once_for_many_node_versions` (três chamadas, uma
  requisição), `a_package_manager_minimum_applies`,
  `an_unreadable_index_is_a_failure_to_know`. Os testes de aplicação existentes
  continuam verdes.
- [ ] **Passo 2: Rodar** `cargo test policy` — esperado: falha.
- [ ] **Passo 3: Implementar**; a aplicação mantém seu comportamento e sua
  saída.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `refactor(policy): decide whether a version is
  allowed in one place`.

### Tarefa 4: Candidatos e a versão permitida mais nova

**Arquivos:**

- Criar: `src/commands/init/candidates.rs` (com testes)
- Modificar: `src/domain/registry.rs` (+ `registry_tests.rs`),
  `src/commands/package_managers/registry.rs`

**Interfaces:**

- Consome: o `Judge` da Tarefa 3; `remote_index::fetch(context, Flavor::Node)`;
  `packument_url`, `registry_base`.
- Produz:
  - `fn published_versions(packument: &str) -> Result<Vec<(Version,
    Option<i64>)>, RegistryError>`: toda chave de `versions` que é
    `major.minor.patch` só com dígitos, com seu dia em `time`;
    pré-lançamentos e `dist-tags` ignorados
  - `pub fn fetch_packument(context: &Context<'_>, tool: Tool, version:
    &Version) -> Result<String, FetchError>` (bytes por
    `third_party_http()`, como `fetch_release_day` faz; `fetch_release_day`
    o reaproveita)
  - `struct Candidate { pub version: Version, pub release: ReleaseInfo }`
  - `fn node_candidates(context: &Context<'_>) -> Result<Vec<Candidate>,
    PolicyError>` (só o flavor node, mais novo primeiro) e
    `fn tool_candidates(context: &Context<'_>, tool: Tool, package_of:
    &Version) -> Result<Vec<Candidate>, PolicyError>` (`package_of` escolhe o
    pacote: yarn `2.0.0` para a busca da mais nova, a versão exata nos demais
    casos; mais novo primeiro)
  - `fn newest_allowed(context: &Context<'_>, judge: &mut Judge, tool: Tool,
    candidates: &[Candidate]) -> Result<Option<Version>, PolicyError>`: para o
    node, só candidatos com nome de LTS; o primeiro sem violação
  - `fn exact(context, judge, tool, candidates, version) ->
    Result<Exact, PolicyError>` com `enum Exact { Allowed, NotPublished,
    NotAllowed(Vec<Violation>) }`
  - Um índice ou registro inalcançável é `PolicyError::ReleaseInfoUnavailable(
    <url>)` (saída 9)
- [ ] **Passo 1: Escrever os testes que falham** — `registry_tests.rs`:
  `prereleases_and_dist_tags_are_ignored` (um packument com `10.0.0-rc.1`,
  `10.8.0`, `10.9.0` e `dist-tags.latest = 10.9.0` sob uma política que nega
  `10.9.0` → a mais nova permitida é `10.8.0` — verificar em candidates.rs),
  `published_versions_reads_the_time_day`. `candidates.rs`:
  `the_newest_allowed_lts_skips_denied_and_too_recent_releases`,
  `newest_node_without_a_policy_is_the_newest_lts`,
  `yarn_candidates_come_from_cli_dist` (URL da requisição),
  `an_exact_yarn_one_reads_the_yarn_package`,
  `an_unpublished_exact_version_is_not_published`,
  `an_unreachable_registry_is_a_failure_to_know`.
- [ ] **Passo 2: Rodar** `cargo test registry candidates` — esperado: falha.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(init): find the newest allowed release of
  each tool`.

### Tarefa 5: O modo de projeto escreve os arquivos

**Arquivos:**

- Criar: `src/commands/init/project.rs`, `src/commands/init/project_tests.rs`
- Modificar: `src/commands/init/mod.rs`, `src/commands/policy/report.rs`
  (tornar o impressor de blocos reutilizável para um sujeito
  `Origin::Argument`, se preciso)

**Interfaces:**

- Consome: Tarefas 1–4; `parse_pins` (conflitos de fixação), `context.fs`
  (`read_to_string`, `write_file`), `context.working_directory()`.
- Produz: `fn run_project(context: &Context<'_>, request: &ProjectRequest)
  -> Result<Output, CliError>` chamada por `init::run` no modo de projeto:
  1. `Judge::load`; para cada ferramenta pedida, na ordem node, npm, yarn,
     pnpm, resolver `Newest` ou checar `Exact` (tudo antes de escrever
     qualquer coisa)
  2. Ler `<cwd>/.nvmrc` (ausente → vazio); recusar yarn sobre uma fixação de
     pnpm ou o inverso: `nvm init: /p/.nvmrc pins pnpm; remove its pnpm= line
     to pin yarn.` (saída 3)
  3. Se yarn ou pnpm foi pedido e `<cwd>/package.json` existe, calcular seu
     novo texto com `set_package_manager` (malformado → a mensagem de malformado
     do `PinsError` do Plano 13, saída 3)
  4. Escrever o `package.json` (quando mudou) e depois o `.nvmrc`
  5. stdout: `Wrote /p/.nvmrc: node v22.11.0, pnpm 9.12.0` e, quando definido,
     `Set packageManager to "pnpm@9.12.0" in /p/package.json`
  - Nenhuma versão permitida: `nvm init: no allowed node LTS release.` /
    `nvm init: no allowed pnpm release.` (saída 7); não publicada: `nvm init:
    pnpm 9.99.0 is not a published release.` / `nvm init: node v20.99.0 is
    not a node release.` (saída 3); não permitida: o bloco do relatório de
    política (`nvm: pnpm 9.1.0 (from the command line) is not allowed by
    policy:`, marcadores, `Use an allowed version.`), saída 7; falha em saber:
    saída 9

- [ ] **Passo 1: Escrever os testes que falham** — `project_tests.rs`:
  `newest_node_and_pnpm_are_written_with_package_manager`,
  `an_existing_nvmrc_keeps_its_other_lines`,
  `an_exact_version_that_is_not_allowed_writes_nothing` (saída 7, arquivos
  intocados), `no_allowed_release_writes_nothing`,
  `yarn_over_a_pnpm_pin_is_refused`, `no_package_json_is_left_absent`,
  `a_failure_to_know_writes_nothing` (saída 9),
  `policy_flags_reach_init` (`nvm --min-pnpm 9.12.0 init --pnpm` pelo harness
  de testes da CLI).
- [ ] **Passo 2: Rodar** `cargo test init` — esperado: falha.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(init): write a project's .nvmrc from the
  policy`.

### Tarefa 6: Documentação

**Arquivos:**

- Modificar: `README.md`, `docs/deviations.md`, o texto de ajuda do `init` em
  `src/cli/commands.rs` (o comentário de documentação que o clap mostra)
- [ ] **Passo 1: Ajuda** — `nvm init --help` e `nvm --help` descrevem o modo de
  projeto (as duas formas de `USAGE`).
- [ ] **Passo 2: README** — uma seção "Starting a project": as flags, a mais
  nova permitida (LTS para o node), versões exatas, o que é escrito (`.nvmrc` no
  diretório de trabalho, `packageManager` só num `package.json` existente),
  conflitos de yarn/pnpm, códigos de saída 3/6/7/9.
- [ ] **Passo 3: deviations.md** — o nvm.sh não tem modo de projeto para
  `init`.
- [ ] **Passo 4: Verificar** — `rumdl fmt` + `rumdl check` nos dois arquivos,
  `cargo test` (testes de ajuda).
- [ ] **Passo 5: Commit** — `docs: describe nvm init project mode`.
