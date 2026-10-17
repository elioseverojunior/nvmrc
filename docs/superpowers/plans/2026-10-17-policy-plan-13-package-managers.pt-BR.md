# nvmrc Plano 13: Plano de implementação de npm, yarn e pnpm

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use
> superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para implementar este plano tarefa por tarefa. Os
> passos usam a sintaxe de checkbox (`- [ ]`) para acompanhamento.

**Objetivo:** Entregar a etapa 3 do design da política de segurança: fixar npm,
yarn e pnpm no `.nvmrc`, instalá-los do registro npm num armazém compartilhado
com checagem de integridade, colocá-los no `PATH` em `use`, `exec` e `run`,
checar `packageManager` no `package.json` e aplicar as regras da política (e os
overrides `--min-npm`, `--min-yarn`, `--min-pnpm`) a cada ferramenta em vigor.

**Arquitetura:** Os tipos de regra só para node passam a ser por ferramenta
(`Tool`, `ToolLayer`, `ToolPolicy`) e o modo combina quatro deles. A parte pura
lê as fixações (`domain/nvmrc/pins.rs`), `packageManager`
(`domain/package_json.rs`), os documentos do registro (`domain/registry.rs`),
as strings de integridade (`domain/integrity.rs`) e edita o `PATH`
(`domain/tool_path.rs`). Os comandos ganham um leitor de fixações
(`commands/project_pins.rs`), um cliente de registro e um armazém
(`commands/package_managers/`), e o portão da política checa todas as
ferramentas num único relatório. `Http` ganha `get_secure_bytes`; `FileSystem`
ganha `make_executable`.

**Stack:** Rust 2024 (MSRV 1.85); `serde_json`, `sha2` (`Sha512`),
`nodejs-semver` e `flate2`/`tar` atrás da porta `Archive` já existentes;
nenhuma dependência nova (base64 é codificado à mão em
`domain/integrity.rs`).

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.pt-BR.md`
(seções 6.2, 6.3, 6.5, 6.7, 6.10, 8; etapa de entrega 3). Parte de
`feat/policy-tools` em `bd1d526` (Planos 11 e 12 integrados).

**Como é o registro** (verificado em 2026-10-17 em
`https://registry.npmjs.org`): `GET /<name>/<version>` devolve o manifesto da
versão com `bin` (um objeto: npm `{npm: bin/npm-cli.js, npx:
bin/npx-cli.js}`, pnpm `{pnpm: bin/pnpm.cjs, pnpx: bin/pnpx.cjs}`, yarn e
`@yarnpkg/cli-dist` `{yarn: bin/yarn.js, yarnpkg: bin/yarn.js}`),
`dist.tarball` (https) e `dist.integrity` (`sha512-<base64>`). `GET /<name>`
devolve o packument, cujo objeto `time` mapeia cada versão para um timestamp
ISO (`2024-03-09T21:35:40.868Z`); ele é grande (npm 26 MB, acima do limite de
texto de 16 MiB), então é buscado como bytes e só quando uma regra
`minimumReleaseAgeDays` precisa dele. Nomes com escopo funcionam tanto como
`@yarnpkg/cli-dist` quanto como `@yarnpkg%2fcli-dist`; este plano sempre
codifica o `/` como `%2f`. Os tarballs se descompactam num único diretório de
topo (`package/`).

## Restrições globais

- Fixações: linhas `npm=`, `yarn=`, `pnpm=` no `.nvmrc` mais próximo, cada uma
  um `major.minor.patch` exato só com dígitos (sem `v`, sem pré-lançamento, sem
  intervalo). Yarn e pnpm juntos é um erro. As fixações são lidas mesmo quando a
  versão do node vem da linha de comando.
- Erros de fixação e a divergência de `packageManager` saem com **3** (o código
  `InvalidVersion`, o `.nvmrc` ou o `package.json` declara uma versão que não é
  válida); violações de política saem com 7; falhas em saber saem com 9.
- `packageManager` só é checado quando yarn ou pnpm está fixado e há um
  `package.json` no mesmo diretório desse `.nvmrc`: deve ser
  `<name>@<version>`, com um sufixo `+<hash>` ignorado. O nvmrc nunca escreve
  o `package.json` neste plano.
- Registro: `NVMRC_NPM_REGISTRY`, padrão `https://registry.npmjs.org`, com uma
  `/` final removida; tudo que não for https é recusado, e toda requisição
  (manifesto, packument, tarball) passa por `get_secure_*`.
- Pacotes: npm → `npm`; pnpm → `pnpm`; yarn abaixo de 2.0.0 → `yarn`; yarn
  2.0.0 e posteriores → `@yarnpkg/cli-dist`.
- Integridade: o tarball deve casar com uma entrada `sha512-` de
  `dist.integrity`; sem entrada `sha512`, ou com divergência, a instalação é
  recusada.
- Armazém: `$NVM_DIR/package-managers/<tool>/<version>/package/` (o pacote
  descompactado) e `.../<version>/bin/<command>` (links simbólicos para
  `../package/<bin path>`, com os alvos tornados executáveis). Uma ferramenta
  está instalada quando `package/package.json` e `bin/<tool>` existem. O
  armazém é compartilhado por todas as versões do node.
- `PATH`: `use`, `exec` e `run` sobre um node instalado põem o `bin` de cada
  ferramenta fixada à frente do do node e descartam qualquer outra entrada de
  `$NVM_DIR/package-managers/`; `deactivate` também as descarta. `system` não
  põe ferramentas fixadas no `PATH`.
- Política: as fontes para npm, yarn e pnpm são a política central, a política
  do repositório, `--min-<tool>` e `NVMRC_MIN_<TOOL>`, combinadas só para
  apertar como no node; `ltsOnly` nunca vale para elas; seus CVEs não são
  checados. Uma ferramenta está em vigor quando fixada; npm sem fixação é o npm
  que vem com o node em uso, consultado só quando existe uma regra de npm.
- Caminhos automáticos (`__auto`, o `use`/`install` do trecho de init) avisam e
  nunca bloqueiam: erros de fixação, instalações fixadas ausentes e violações
  de ferramenta são avisos.
- Sem fixações e sem política → comportamento exatamente o de hoje; a suíte de
  compatibilidade com nvm.sh passa sem mudanças.
- Arquivos com menos de 300 linhas, funções com menos de 30, complexidade
  abaixo de 10, sem unwrap/expect em entrada externa, `cargo fmt`, `cargo clippy
  --all-targets --all-features -- -D warnings`, `rumdl` no Markdown alterado,
  sem rede nos testes, TDD; commits Conventional, assinados com GPG (`git
  commit -S`), sem atribuição de IA.

## Foco da revisão

- Um manifesto cujo `bin` aponta para fora do pacote (`../../evil`, `/etc/x`)
  ou nomeia um comando com `/`: recusado antes de qualquer escrita (teste da
  Tarefa 4 `a_bin_path_outside_the_package_is_refused`).
- Um tarball cujo sha512 difere de `dist.integrity`, ou uma integridade só com
  `sha1-`: instalação recusada, nada deixado no armazém (teste da Tarefa 5
  `a_tarball_that_fails_integrity_leaves_nothing_in_the_store`).
- `pnpm=9.12`, `pnpm=v9.12.0`, `pnpm=latest` no `.nvmrc`: saída 3 nomeando o
  arquivo e a linha, e a leitura de versão pura do nvm.sh não muda (teste da
  Tarefa 3 `a_partial_pin_is_rejected_with_the_file_and_line`).
- `use` a partir de um projeto que fixa pnpm para um sem fixações: o `bin` do
  pnpm antigo sai do `PATH` (teste da Tarefa 8
  `use_drops_the_tool_bins_of_another_project`).
- Um `packageManager` `pnpm@9.12.0+sha512.abc` contra `pnpm=9.12.0`: passa;
  nenhum `package.json`: passa (teste da Tarefa 3
  `a_corepack_hash_suffix_is_ignored`).

---

### Tarefa 1: Tipos de regra por ferramenta

**Arquivos:**

- Criar: `src/domain/tool.rs` (com seu `#[cfg(test)] mod tests`)
- Modificar: `src/domain/mod.rs`, `src/domain/policy/rules.rs`,
  `src/domain/policy/verdict.rs`, `src/domain/policy/mod.rs`,
  `src/domain/policy/rules_tests.rs`, `src/domain/policy/verdict_tests.rs`,
  todo usuário de `NodeLayer`/`NodePolicy`/`merge` em `src/commands/policy/`

**Interfaces:**

- Produz:
  - `enum Tool { Node, Npm, Yarn, Pnpm }` (`Debug, Clone, Copy, PartialEq,
    Eq, Ord, PartialOrd`) com `pub const PACKAGE_MANAGERS: [Tool; 3]`
    (`Npm, Yarn, Pnpm`), `fn name(self) -> &'static str` (`node`, `npm`,
    `yarn`, `pnpm`) e `fn show(self, version: &Version) -> String` (node:
    `v20.11.1`, os outros: `10.9.0`)
  - `ToolLayer` (renomeado de `NodeLayer`, mesmos campos) com
    `fn from_node(source, &NodeRules) -> Result<Self, DocumentError>`,
    `fn from_package(tool: Tool, source, &PackageRules) -> Result<Self,
    DocumentError>` (nomes de campo `tools.<tool>.minimum`, `.allow`,
    `.deny[<i>]`; `lts_only` `None`) e `fn minimum_only(source, Version)`
  - `ToolPolicy` (renomeado de `NodePolicy`) ganha `pub tool: Tool`; `Default`
    é removido; `fn unrestricted(tool: Tool) -> Self` e `fn
    is_unrestricted(&self) -> bool` (nenhuma regra)
  - `fn merge(tool: Tool, layers: Vec<ToolLayer>) -> (ToolPolicy, Vec<String>)`
  - `Violation::BelowMinimum(Sourced<String>)`: o mínimo como
    `tool.show(..)`; `Violation::NotNode` é produzido só para `Tool::Node`
- [ ] **Passo 1: Escrever os testes que falham** — em `src/domain/tool.rs`:
  `shows_node_with_a_v_and_package_managers_without` (`Tool::Node.show(v)` é
  `"v20.11.1"`, `Tool::Npm.show(v)` é `"10.9.0"`). Em `rules_tests.rs`:
  `a_package_layer_names_its_own_fields` (`deny: ["10.x"]` sob npm falha com o
  campo `tools.npm.deny[0]`), `package_minimums_tighten_like_node` (duas
  camadas de npm `10.2.4` e `10.8.0` → vencedor `10.8.0`, o override mais
  frouxo avisa). Em `verdict_tests.rs`: `an_npm_minimum_is_shown_without_a_v`
  (texto da violação `below the minimum 10.2.4 (from policy p)`). Manter toda
  expectativa existente do node (`below the minimum v20.11.1 (from
  --min-node)`).
- [ ] **Passo 2: Rodar** `cargo test domain::` — esperado: falha (sem `Tool`).
- [ ] **Passo 3: Implementar** as renomeações e o campo `tool`, depois
  corrigir todo usuário (`commands/policy/mode.rs` passa `Tool::Node`).
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa, nenhuma mensagem do
  node muda.
- [ ] **Passo 5: Commit** — `refactor(policy): make the rule types per tool`.

### Tarefa 2: Regras de npm, yarn e pnpm no modo e nas flags

**Arquivos:**

- Modificar: `src/domain/policy/flags.rs` (`POLICY_FLAGS`),
  `src/commands/policy/mode.rs` (dividir em auxiliares; mover as camadas de
  pacote para um novo `src/commands/policy/tool_layers.rs` se `mode.rs`
  passar de 250 linhas), `src/commands/policy/tests/mode.rs`,
  `src/shell/init/tests.rs`, `src/cli/tests/policy.rs`

**Interfaces:**

- Consome: `Tool`, `ToolLayer::from_package` e `merge(tool, ..)` da Tarefa 1.
- Produz:
  - `POLICY_FLAGS: [PolicyFlag; 6]`, acrescentadas nesta ordem:
    `--min-npm`/`NVMRC_MIN_NPM`, `--min-yarn`/`NVMRC_MIN_YARN`,
    `--min-pnpm`/`NVMRC_MIN_PNPM`
  - `EnforcedPolicy { pub node: ToolPolicy, pub npm: ToolPolicy, pub yarn:
    ToolPolicy, pub pnpm: ToolPolicy, pub cve: CvePolicy }` com
    `fn tool(&self, tool: Tool) -> &ToolPolicy`
  - Para cada gerenciador de pacotes as camadas são, em ordem: política
    central, política do repositório, flag, variável (um empate cita a
    política central).
- [ ] **Passo 1: Escrever os testes que falham** — em `tests/mode.rs`:
  `npm_rules_merge_from_the_policy_and_the_variable` (`npm.minimum 10.2.4`
  central, `NVMRC_MIN_NPM=10.8.0` → mínimo de npm `10.8.0` vindo de
  `NVMRC_MIN_NPM`), `a_min_pnpm_flag_alone_turns_policy_mode_on`,
  `a_blank_min_yarn_variable_is_unset`,
  `an_invalid_min_npm_flag_is_a_failure_to_know` (`--min-npm=ten` → saída 9
  nomeando `--min-npm`), `yarn_rules_from_the_repository_policy_apply`. Em
  `shell/init/tests.rs`: as funções POSIX e fish roteiam `nvm --min-pnpm 9 use
  20` e `nvm --min-yarn=4 use 20` como `use`. Em `cli/tests/policy.rs`: `nvm
  --min-npm 10 version 20` remove a flag.
- [ ] **Passo 2: Rodar** `cargo test policy` — esperado: falhas.
- [ ] **Passo 3: Implementar** as flags, as camadas de pacote e
  `EnforcedPolicy`; manter `resolve_mode` com menos de 30 linhas.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(policy): merge npm, yarn and pnpm rules and
  their minimum flags`.

### Tarefa 3: Fixações no `.nvmrc` e a checagem de `packageManager`

**Arquivos:**

- Criar: `src/domain/nvmrc/pins.rs`, `src/domain/nvmrc/pins_tests.rs`,
  `src/domain/package_json.rs` (com testes), `src/commands/project_pins.rs`
  (com testes)
- Modificar: `src/domain/nvmrc/mod.rs` (compartilhar as regras de comentário e
  de aparar espaços), `src/domain/mod.rs`, `src/commands/mod.rs`,
  `src/error.rs`

**Interfaces:**

- Consome: `Tool` da Tarefa 1.
- Produz:
  - `struct Pins { pub npm: Option<Version>, pub yarn: Option<Version>, pub
    pnpm: Option<Version> }` (`Default`) com `fn iter(&self) -> impl
    Iterator<Item = (Tool, Version)>` (ordem npm, yarn, pnpm),
    `fn get(&self, tool: Tool) -> Option<Version>` e
    `fn package_manager(&self) -> Option<(Tool, Version)>` (yarn ou pnpm)
  - `fn parse_pins(contents: &str) -> Result<Pins, PinError>`; `PinError::
    NotExact { line: String }` e `PinError::YarnAndPnpm`. As linhas são as que
    `process_content` vê (mesmas regras de `#` e de aparar espaços, um auxiliar
    compartilhado); chaves diferentes de `npm`, `yarn`, `pnpm` são ignoradas
  - `fn check_package_manager(text: &str, tool: Tool, version: &Version) ->
    Result<(), PackageManagerError>`; `PackageManagerError::{Malformed(String),
    Missing, Different { found: String }}`
  - `struct ProjectPins { pub nvmrc: Option<PathBuf>, pub pins: Pins }`
    (`Default`)
  - `fn read(context: &Context<'_>) -> Result<ProjectPins, CliError>`: o
    `.nvmrc` mais próximo a partir do diretório de trabalho (nenhum → padrão),
    suas fixações, e o `package.json` ao lado dele checado quando existe uma
    fixação de yarn ou pnpm
  - `fn read_for(context: &Context<'_>, transcript: &mut Transcript) ->
    Result<ProjectPins, CliError>`: como `read`, mas num contexto consultivo um
    erro é impresso como `nvm: warning: <message>` e o resultado é vazio
  - `CliError::Pins(PinsError)` → `NvmExitCode::InvalidVersion`, onde
    `PinsError` (em `error.rs`) guarda os caminhos e imprime, um por caso:

    ```text
    nvm: /p/.nvmrc: 'pnpm=9.12' is not an exact version (major.minor.patch).
    nvm: /p/.nvmrc pins both yarn and pnpm; pin at most one.
    nvm: /p/package.json has no packageManager; set it to "pnpm@9.12.0" to match /p/.nvmrc.
    nvm: /p/package.json has packageManager "yarn@4.5.0", but /p/.nvmrc pins yarn 4.5.1; set it to "yarn@4.5.1".
    nvm: /p/package.json cannot be read as JSON: <reason>
    ```

- [ ] **Passo 1: Escrever os testes que falham** — `pins_tests.rs`:
  `reads_the_three_pins` (`"20.11.1\nnpm=10.9.0\npnpm=9.12.0\n"`),
  `a_partial_pin_is_rejected_with_the_file_and_line` (`pnpm=9.12`,
  `pnpm=v9.12.0`, `pnpm=latest`, cada um → `NotExact` com aquela linha),
  `yarn_and_pnpm_together_are_rejected`, `comments_and_spaces_are_ignored`
  (`" npm = 10.9.0 # pinned"`), `other_keys_are_ignored`; mais um teste de que
  `process_content` ainda devolve a versão pura para um arquivo com fixações.
  `package_json.rs`: `a_matching_package_manager_passes`,
  `a_corepack_hash_suffix_is_ignored`, `a_missing_field_is_reported`,
  `another_name_or_version_is_different` (`found` guarda o campo como foi
  escrito), `malformed_json_is_reported`. `project_pins.rs`:
  `no_nvmrc_means_no_pins`, `no_package_json_passes`,
  `a_divergent_package_json_exits_3`,
  `an_advisory_context_warns_and_returns_no_pins`.
- [ ] **Passo 2: Rodar** `cargo test pins package_json` — esperado: falhas.
- [ ] **Passo 3: Implementar** as funções de domínio e o leitor.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(nvmrc): read npm, yarn and pnpm pins and
  check packageManager`.

### Tarefa 4: Documentos do registro, integridade e downloads seguros de bytes

**Arquivos:**

- Criar: `src/domain/registry.rs` (+ `registry_tests.rs`),
  `src/domain/integrity.rs` (com testes),
  `src/commands/package_managers/mod.rs`,
  `src/commands/package_managers/registry.rs` (com testes)
- Modificar: `src/ports/http.rs`, `src/adapters/ureq_http/mod.rs` (+ testes),
  `src/adapters/retrying_http.rs`, `src/adapters/no_http.rs`,
  `src/fakes/http.rs`, `src/domain/mod.rs`, `src/commands/mod.rs`

**Interfaces:**

- Consome: `Tool` da Tarefa 1.
- Produz:
  - `Http::get_secure_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>`:
    `get_bytes` com o limite de tamanho de download e a regra https de
    `get_secure_text` (a URL e todo redirecionamento); o cliente com novas
    tentativas a repete como `get_bytes`; o fake espelha `get_secure_text`
  - `fn package_name(tool: Tool, version: &Version) -> &'static str`
  - `fn manifest_url(registry: &str, tool: Tool, version: &Version) -> String`
    (`<registry>/<name>/<version>`, `/` no nome como `%2f`) e
    `fn packument_url(registry: &str, tool: Tool, version: &Version) -> String`
  - `struct Manifest { pub tarball: String, pub integrity: String, pub
    commands: Vec<(String, String)> }` e
    `fn parse_manifest(text: &str) -> Result<Manifest, RegistryError>`: `bin`
    deve ser um objeto não vazio; o nome de um comando deve ser não vazio e sem
    `/`; um alvo deve ser relativo, sem componente `..` ou raiz
  - `fn release_day(packument: &str, version: &Version) -> Result<Option<i64>,
    RegistryError>`: a parte de data de `time[<version>]` como dias desde
    1970-01-01 (`days_from_iso_date`); `None` quando ausente
  - `fn verify(integrity: &str, bytes: &[u8]) -> Result<(), IntegrityError>`:
    entradas separadas por espaço, só as `sha512-` contam; o base64 (alfabeto
    padrão, com padding) do SHA-512 calculado deve ser igual a uma delas;
    `IntegrityError::{NoSha512, Mismatch}`
  - `fn registry_base(context: &Context<'_>) -> Result<String, FetchError>`
  - `fn fetch_manifest(context: &Context<'_>, tool: Tool, version: &Version)
    -> Result<Manifest, FetchError>` (via `context.http()`)
  - `fn fetch_release_day(context: &Context<'_>, http: &dyn Http, tool: Tool,
    version: &Version) -> Result<Option<i64>, FetchError>`
  - `struct FetchError { pub location: String, pub reason: String }`
    exibido como `<location>: <reason>`
- [ ] **Passo 1: Escrever os testes que falham** — `registry_tests.rs`:
  `yarn_one_and_berry_come_from_different_packages` (`1.22.22` → `yarn`,
  `2.0.0` e `4.5.0` → `@yarnpkg/cli-dist`),
  `scoped_names_are_encoded_in_urls`
  (`https://r/@yarnpkg%2fcli-dist/4.5.0`), `parses_the_npm_manifest` (o
  formato real citado acima), `a_bin_path_outside_the_package_is_refused`
  (`../../evil`, `/etc/x`, e o comando `a/b`), `a_missing_bin_is_refused`,
  `release_day_reads_the_time_entry` (`2024-03-09T21:35:40.868Z` →
  `days_from_iso_date("2024-03-09")`). `integrity.rs`:
  `base64_matches_rfc4648_vectors` (`""`, `"f"`, `"fo"`, `"foo"`, `"foob"`,
  `"fooba"`, `"foobar"`), `a_matching_sha512_passes`,
  `a_sha1_only_integrity_is_refused`, `a_mismatch_is_refused`.
  `registry.rs` (commands): `the_default_registry_is_npmjs`,
  `a_trailing_slash_is_trimmed`, `an_http_registry_is_refused`.
  `ureq_http/tests.rs` e `fakes/http.rs`: `get_secure_bytes` recusa uma URL
  `http://` e um redirecionamento para uma.
- [ ] **Passo 2: Rodar** `cargo test registry integrity http` — esperado:
  falhas.
- [ ] **Passo 3: Implementar** o método da porta em todo adaptador, depois o
  domínio e o cliente.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(http): download bytes over https only` e
  `feat(registry): read npm registry manifests and verify integrity`.

### Tarefa 5: O armazém de pacotes

**Arquivos:**

- Criar: `src/commands/package_managers/store.rs`,
  `src/commands/package_managers/store_tests.rs`
- Modificar: `src/ports/fs.rs`, `src/adapters/std_fs/mod.rs` (+ testes),
  `src/fakes/file_system/mod.rs`, `src/commands/package_managers/mod.rs`

**Interfaces:**

- Consome: `fetch_manifest`, `verify`, `Manifest` da Tarefa 4;
  `commands::install::place::unpack`.
- Produz:
  - `FileSystem::make_executable(&self, path: &Path) -> io::Result<()>`:
    acrescenta os bits de execução onde há bits de leitura (Unix); o fake marca
    o caminho como executável
  - `fn tool_dir(context: &Context<'_>, tool: Tool, version: &Version) ->
    Result<PathBuf, CliError>` (`$NVM_DIR/package-managers/<tool>/<version>`)
    e `fn bin_dir(..) -> Result<PathBuf, CliError>` (`<tool_dir>/bin`)
  - `fn is_installed(context: &Context<'_>, tool: Tool, version: &Version) ->
    bool`
  - `fn install_tool(context: &Context<'_>, tool: Tool, version: &Version) ->
    Result<(), String>`: manifesto → tarball por https → integridade → os
    bytes escritos em `<tool_dir>.partial/download.tgz` → descompactados em
    `<tool_dir>.partial/files` → o diretório de topo movido para
    `<tool_dir>.partial/out/package` → um link simbólico por comando em
    `out/bin` → alvos tornados executáveis → um `<tool_dir>` existente
    removido → `out` renomeado para `<tool_dir>` → `.partial` removido.
    Qualquer falha remove `.partial` e deixa `<tool_dir>` como estava.
- [ ] **Passo 1: Escrever os testes que falham** — `store_tests.rs` (HTTP
  fake, `FakeArchive`, FS fake): `installs_pnpm_with_its_commands_linked`
  (`bin/pnpm` → `../package/bin/pnpm.cjs`, `bin/pnpx`, alvo executável,
  `is_installed` verdadeiro), `yarn_berry_comes_from_cli_dist` (requisição a
  `.../@yarnpkg%2fcli-dist/4.5.0`),
  `a_tarball_that_fails_integrity_leaves_nothing_in_the_store`,
  `a_broken_install_is_replaced`, `the_registry_variable_is_used`. Testes de
  `std_fs`: `make_executable_adds_execute_bits` (`0o644` → `0o755`).
- [ ] **Passo 2: Rodar** `cargo test store make_executable` — esperado:
  falhas.
- [ ] **Passo 3: Implementar** o método da porta e o armazém.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(store): install npm, yarn and pnpm into a
  shared store`.

### Tarefa 6: `install` instala as ferramentas fixadas

**Arquivos:**

- Criar: `src/commands/install/pinned.rs`,
  `src/commands/install/tests/pinned.rs`
- Modificar: `src/commands/install/mod.rs`, `src/commands/install/tests/mod.rs`

**Interfaces:**

- Consome: `read_for`, `ProjectPins` da Tarefa 3; `is_installed`,
  `install_tool` da Tarefa 5.
- Produz:
  - `install()` lê as fixações uma vez com `read_for` logo depois das opções,
    antes de resolver (um erro de fixação encerra a instalação, saída 3, antes
    de qualquer download)
  - `fn install_pinned(context: &Context<'_>, pins: &Pins, offline: bool,
    transcript: &mut Transcript) -> Step<()>`, chamada tanto em
    `already_installed` quanto em `fresh_install` depois que o node está no
    lugar e antes de ele ser ativado. Por ferramenta fixada, na ordem de
    `Pins::iter`: instalada → `pnpm 9.12.0 is already installed.`; senão
    offline → `pnpm 9.12.0 is not installed, and --offline does not download.`
    e saída 1; senão `Installing pnpm 9.12.0...` e então `install_tool`, uma
    falha impressa como `nvm: cannot install pnpm 9.12.0: <reason>` e saída 1
- [ ] **Passo 1: Escrever os testes que falham** — `tests/pinned.rs`:
  `install_installs_the_pinned_tools_after_node`,
  `an_installed_tool_is_not_downloaded_again`,
  `offline_with_a_missing_tool_fails`,
  `pins_come_from_nvmrc_even_with_a_version_argument`,
  `a_pin_error_stops_before_any_download` (nenhuma requisição HTTP
  registrada), `no_pins_change_nothing` (a transcrição de instalação
  existente, byte a byte).
- [ ] **Passo 2: Rodar** `cargo test install::` — esperado: falhas.
- [ ] **Passo 3: Implementar** `install_pinned` e as chamadas.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(install): install the npm, yarn and pnpm
  pins`.

### Tarefa 7: O portão da política checa toda ferramenta em vigor

**Arquivos:**

- Criar: `src/commands/policy/tools.rs`, `src/commands/policy/report.rs`
  (o relatório tirado de `enforce.rs`), `src/commands/policy/bundled.rs`,
  `src/commands/policy/tests/tools.rs`
- Modificar: `src/commands/policy/enforce.rs`, `src/commands/policy/gate.rs`,
  `src/commands/policy/mod.rs`, `src/domain/index.rs`,
  `src/commands/install/resolve.rs`, `src/commands/use_version/mod.rs`,
  `src/commands/exec/launch.rs`

**Interfaces:**

- Consome: `EnforcedPolicy::tool` da Tarefa 2; `ProjectPins` da Tarefa 3;
  `fetch_release_day` da Tarefa 4.
- Produz:
  - `Release` ganha `pub npm: Option<Version>` (coluna 4 do índice, `npm`)
  - `Requested` ganha `pub pins: ProjectPins`
  - `gate_installed(context, version, origin, pins: &ProjectPins,
    transcript)` e `gate_system(context, pins: &ProjectPins, transcript)`;
    `use` e `exec` (e portanto `run`) leem as fixações uma vez com `read_for` e
    as repassam; `install` passa as fixações da Tarefa 6
  - `enum ToolOrigin { Nvmrc(PathBuf), Bundled(Version) }`
  - `fn tool_violations(context: &Context<'_>, requested: &Requested, policy:
    &EnforcedPolicy) -> Result<Vec<(Tool, Version, ToolOrigin,
    Vec<Violation>)>, PolicyError>`: cada ferramenta fixada; npm sem fixação
    só quando `!policy.npm.is_unrestricted()`; dias de lançamento vindos do
    registro (cliente da política) só para uma ferramenta com
    `minimumReleaseAgeDays`, indisponível ou offline →
    `PolicyError::ReleaseInfoUnavailable(<url or "offline">)`
  - `fn bundled_npm(context: &Context<'_>, requested: &Requested) ->
    Result<Version, PolicyError>`: o `version` de
    `<node dir>/lib/node_modules/npm/package.json` quando esse node está
    instalado; para o node do sistema, `npm --version` do `npm` ao lado do
    `node` do sistema; senão a coluna `npm` do índice (offline →
    `ReleaseInfoUnavailable("offline")`); nada legível →
    `ReleaseInfoUnavailable`
  - O relatório imprime um bloco por ferramenta infratora, node primeiro,
    depois npm, yarn, pnpm:
    `nvm: npm 10.1.0 (from /p/.nvmrc) is not allowed by policy:` /
    `nvm: npm 10.1.0 (bundled with node v20.10.0) is not allowed by policy:`,
    os marcadores, depois a correção: `Raise /p/.nvmrc to an allowed version.`
    ou `Pin an allowed npm in .nvmrc (npm=<version>).`; os prefixos consultivos
    `nvm: warning:` e passam; qualquer violação para com saída 7
- [ ] **Passo 1: Escrever os testes que falham** — `tests/tools.rs`:
  `a_pinned_pnpm_below_the_minimum_exits_7`,
  `a_tool_that_is_not_pinned_is_not_checked` (regra de yarn, sem fixação de
  yarn → passa, nenhuma requisição),
  `bundled_npm_of_an_installed_node_is_read_from_its_package_json`,
  `bundled_npm_for_install_comes_from_the_index`,
  `bundled_npm_is_not_looked_up_without_an_npm_rule`,
  `an_age_rule_fetches_the_packument_and_fails_when_too_recent`,
  `an_unreachable_registry_with_an_age_rule_exits_9`,
  `offline_with_an_age_rule_exits_9`,
  `node_and_pnpm_violations_are_reported_together`,
  `the_auto_path_warns_about_a_tool_and_passes`. Em `domain/index.rs`:
  `reads_the_npm_column`.
- [ ] **Passo 2: Rodar** `cargo test policy` — esperado: falhas.
- [ ] **Passo 3: Implementar** as checagens de ferramenta, a consulta do npm
  que vem com o node, o relatório e as mudanças nos pontos de chamada.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(policy): enforce the rules of npm, yarn and
  pnpm`.

### Tarefa 8: Ferramentas fixadas no `PATH`

**Arquivos:**

- Criar: `src/domain/tool_path.rs` (com testes),
  `src/commands/package_managers/path.rs` (com testes)
- Modificar: `src/commands/use_version/apply/mod.rs`,
  `src/commands/use_version/mod.rs`, `src/commands/exec/launch.rs`,
  `src/commands/deactivate/mod.rs`, `src/error.rs`
  (`PinsError::NotInstalled`), seus testes

**Interfaces:**

- Consome: `ProjectPins` da Tarefa 3; `bin_dir`, `is_installed` da Tarefa 5;
  a leitura das fixações em `use` e `exec` da Tarefa 7.
- Produz:
  - `fn strip_tool_bins(path: &str, nvm_dir: &str) -> String`: descarta toda
    entrada que começa com `<nvm_dir>/package-managers/`
  - `fn prepend(path: &str, directories: &[String]) -> String`
  - `fn pinned_path(context: &Context<'_>, path: &str, pins: &ProjectPins,
    transcript: &mut Transcript) -> Result<String, CliError>`: remove, depois
    antepõe o `bin` de cada ferramenta fixada e instalada na ordem de
    `Pins::iter`. Uma ferramenta fixada que não está instalada: explícito →
    `CliError::Pins(PinsError::NotInstalled { .. })` imprimindo
    `nvm: pnpm 9.12.0 (from /p/.nvmrc) is not installed. Run \`nvm install\`.`
    (saída 3); consultivo → esse texto como aviso, a ferramenta deixada de
    fora
  - `use` (instalado) e `exec`/`run` (instalado) passam o `PATH` trocado por
    `pinned_path` antes do script, do sufixo do npm e do ambiente do filho;
    `deactivate` também aplica `strip_tool_bins` ao `PATH` sem mudar suas
    mensagens
- [ ] **Passo 1: Escrever os testes que falham** — `tool_path.rs`:
  `strips_only_package_manager_entries`, `prepends_in_order`.
  `path.rs`: `pins_go_ahead_of_node`, `a_missing_tool_fails_explicitly`,
  `a_missing_tool_is_a_warning_when_advisory`. Testes de `use`:
  `use_puts_pinned_pnpm_on_path` (o `PATH` exportado começa com o `bin` do
  pnpm, depois o do node), `use_drops_the_tool_bins_of_another_project`,
  `now_using_shows_the_pinned_npm`. Testes de `exec`:
  `exec_gives_the_child_the_pinned_tools`. Testes de `deactivate`:
  `deactivate_drops_tool_bins`.
- [ ] **Passo 2: Rodar** `cargo test use_version exec deactivate tool_path` —
  esperado: falhas.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa; os testes de
  compatibilidade com nvm.sh sem mudanças.
- [ ] **Passo 5: Commit** — `feat(use): put the pinned npm, yarn and pnpm on
  PATH`.

### Tarefa 9: Documentação

**Arquivos:**

- Modificar: `README.md`, `docs/deviations.md`
- [ ] **Passo 1: README** — uma seção "npm, yarn and pnpm": as linhas de
  fixação, a regra da versão exata, yarn ou pnpm mas não os dois, a checagem
  de `packageManager`, o caminho do armazém, `NVMRC_NPM_REGISTRY` (só https),
  integridade, ordem do `PATH`, `system` sem fixações, comportamento offline;
  acrescentar `--min-npm`, `--min-yarn`, `--min-pnpm` e suas variáveis à seção
  de política; código de saída 3 para erros de fixação e de `packageManager`.
- [ ] **Passo 2: deviations.md** — as linhas `npm=`/`yarn=`/`pnpm=` mudam o
  que `use`, `exec`, `run`, `install` e `deactivate` fazem (o nvm.sh as
  ignora); a divergência de `packageManager` sai com 3; o armazém e as
  entradas de `PATH`.
- [ ] **Passo 3: Verificar** — `rumdl fmt README.md docs/deviations.md`, `rumdl
  check README.md docs/deviations.md`, `cargo test`.
- [ ] **Passo 4: Commit** — `docs: describe npm, yarn and pnpm pins and their
  policy`.
