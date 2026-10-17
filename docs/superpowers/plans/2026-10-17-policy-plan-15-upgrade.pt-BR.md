# nvmrc Plano 15: Plano de implementação do `nvm upgrade`

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use
> superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para implementar este plano tarefa por tarefa. Os
> passos usam a sintaxe de checkbox (`- [ ]`) para acompanhamento.

**Objetivo:** Entregar a etapa 5 do design da política de segurança: `nvm
upgrade [--node] [--npm] [--yarn] [--pnpm] [--major[=<N>]]` leva as
ferramentas fixadas pelo `.nvmrc` mais próximo à versão permitida mais nova —
dentro da major atual, a menos que se peça o contrário —, reescreve `.nvmrc` e
`packageManager` e depois instala.

**Arquitetura:** As peças que o `nvm init` usa para escolher e escrever versões
passam para um módulo compartilhado `commands/project/` (candidatos, a seleção
guiada pelo juiz, o gravador de arquivos com sua restauração).
`commands/upgrade/` interpreta as flags, lê as versões atuais do `.nvmrc` mais
próximo, escolhe um alvo por ferramenta selecionada, recusa antes de escrever
qualquer coisa quando alguma ferramenta não tem alvo, escreve, imprime cada
mudança e executa o `install` para as novas versões.

**Stack:** Rust 2024 (MSRV 1.85); nenhuma dependência nova.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.pt-BR.md`
(seções 6.2, 6.5, 6.7, 6.9, 6.10; etapa de entrega 5). Parte de
`feat/policy-upgrade` em `22856d1` (Planos 11–14 integrados).

## Restrições globais

- O `nvm upgrade` lê o `.nvmrc` **mais próximo** (subindo pelos diretórios,
  como o `use`) e escreve esse arquivo e o `package.json` ao lado dele.
- Sem flags de ferramenta, toda ferramenta fixada pelo `.nvmrc` é selecionada
  (o node está fixado quando o arquivo tem uma linha de versão simples; npm,
  yarn e pnpm, quando têm uma linha `<tool>=`). Com flags de ferramenta, só
  essas; uma flag para uma ferramenta que o arquivo não fixa é um erro de uso,
  saída 6 (`nvm upgrade: /p/.nvmrc does not pin pnpm; use nvm init --pnpm to
  add it.`).
- Padrão: cada ferramenta vai para a versão permitida mais nova (spec 6.2)
  **dentro da major que já usa**; nunca cruza uma major por conta própria.
  `--major`: a versão permitida mais nova em qualquer major. `--major=<N>`: a
  versão permitida mais nova na major `N`, e um erro de uso (saída 6) a menos
  que exatamente uma ferramenta esteja selecionada.
- Para o node, "versão permitida mais nova" é qualquer versão permitida (um
  nome de LTS só é exigido quando a política define `ltsOnly`); npm, yarn e
  pnpm usam apenas versões estáveis `major.minor.patch`. O yarn lê o pacote
  `yarn` para a major 1 e `@yarnpkg/cli-dist` para a major 2 em diante
  (`--major` pesquisa `@yarnpkg/cli-dist`).
- Uma linha de node que é uma versão parcial (`20`, `v20.11`) usa essa major e
  é substituída pelo alvo exato. Uma linha de node que não é uma versão (um
  alias como `lts/*`, `node`, uma versão `iojs-`) é recusada, saída 3 (`nvm
  upgrade: /p/.nvmrc names node as "lts/*", not a version; use nvm init --node
  to pin one.`). Nenhum `.nvmrc`: saída 3 (`nvm upgrade: no .nvmrc found; run
  nvm init first.`). Uma linha de fixação inválida: o erro de fixação do Plano
  13, saída 3.
- Uma ferramenta que já está no alvo é deixada como está e informada (`pnpm
  9.12.0 is up to date`). Uma versão atual que não é permitida vai para o alvo
  mesmo quando o alvo é menor.
- Nenhum alvo permitido na major: falhar antes de escrever qualquer coisa,
  saída 7. Imprimir, para cada ferramenta nessa situação, `nvm upgrade: no
  allowed <tool> release in major <M>.`, o bloco de política da versão mais
  nova nessa major com as regras que ela quebra e suas origens (spec 6.10 — o
  que sobrou do Plano 14), e então `The lowest major with an allowed release is
  <L>. Run \`nvm upgrade --<tool> --major=<L>\`, or \`nvm upgrade --<tool>
  --major\` for the newest.` (ou `No major has an allowed <tool> release.`). Se
  qualquer ferramenta selecionada não tem alvo, nada é escrito.
- As mudanças são impressas como `node v20.11.1 -> v20.18.0` / `pnpm 9.1.0 ->
  9.12.0` (`Tool::show`), na ordem node, npm, yarn, pnpm. Nada mudou: saída 0,
  nada escrito, nada instalado.
- Depois de escrever, o `install` roda como o `nvm install` rodaria sem
  argumento (lê o `.nvmrc` reescrito, instala o node e as fixações). Se ele
  falhar, sua saída e seu status são mantidos e uma linha é acrescentada: `nvm
  upgrade: /p/.nvmrc is updated, but the install failed; run nvm install to
  finish.`
- A escrita segue o Plano 14: os dois textos são calculados primeiro; o
  `package.json` é restaurado quando o `.nvmrc` não pode ser escrito; a linha
  de falha na restauração nomeia `nvm upgrade`.
- Códigos de saída: 6 uso, 3 problemas de arquivo acima, 7 nenhum alvo
  permitido, 9 falha em saber, 1 E/S, o status do install quando o install
  falha.
- O comportamento do `nvm init` não muda com a refatoração (seus testes
  continuam verdes).
- Arquivos com menos de 300 linhas, funções com menos de 30 (da assinatura à
  chave de fechamento), complexidade abaixo de 10, sem unwrap/expect em
  entrada externa, `cargo fmt`, `cargo clippy --all-targets --all-features --
  -D warnings`, `rumdl` no Markdown alterado, sem rede nos testes, TDD;
  commits Conventional, assinados com GPG (`git commit -S`), sem atribuição de
  IA.

## Foco da revisão

- Uma linha de node parcial `20`: atualizada dentro da major 20 para um
  `v20.x.y` exato (teste da Tarefa 4
  `a_partial_node_line_is_upgraded_within_its_major`).
- Um `.nvmrc` com `lts/*`: saída 3, nada escrito, nenhuma requisição (teste da
  Tarefa 4 `an_alias_node_line_is_refused`).
- `yarn=1.22.19`: a atualização padrão fica no pacote `yarn` na major 1;
  `--yarn --major` passa para `@yarnpkg/cli-dist` (teste da Tarefa 3
  `yarn_one_stays_on_yarn_and_major_moves_to_cli_dist`).
- Uma versão atual que a política agora nega, mais nova que toda versão
  permitida na sua major: desce para a permitida mais nova e imprime a mudança,
  não "up to date" (teste da Tarefa 3
  `a_denied_current_version_moves_to_the_newest_allowed`).
- Duas ferramentas selecionadas, uma sem alvo: nada escrito, toda ferramenta
  com falha informada, saída 7 (teste da Tarefa 4
  `one_tool_without_a_target_writes_nothing`).

---

### Tarefa 1: Compartilhar a seleção e o gravador

**Arquivos:**

- Criar: `src/commands/project/mod.rs`
- Mover: `src/commands/init/candidates.rs` (+ `candidates_tests.rs`),
  `src/commands/init/write.rs` (+ `write_tests.rs`) e o enum `Stop` para
  `src/commands/project/`; o `init` importa tudo de lá
- Modificar: `src/commands/init/{mod.rs,project.rs,project_tests.rs,test_support.rs}`,
  `src/commands/mod.rs`

**Interfaces:**

- Produz (tudo em `crate::commands::project`):
  - tudo o que o `init` já usa, sem mudança de comportamento
  - `fn first_allowed(context, judge: &mut Judge, tool, candidates:
    &[Candidate]) -> Result<Option<Candidate>, PolicyError>` (sem filtro de
    LTS); `newest_allowed` passa a ser a chamada dela com filtro de LTS para o
    node
  - `fn in_major(candidates: &[Candidate], major: u64) -> Vec<Candidate>`
  - `fn majors(candidates: &[Candidate]) -> Vec<u64>` (ascendente, distintas)
  - `save(context, command: &str, files, nvmrc_text, package_edit,
    transcript)`: `command` (`"nvm init"`, `"nvm upgrade"`) prefixa a linha de
    falha na restauração
  - `ProjectFiles::beside(nvmrc: &Path) -> ProjectFiles` (o `.nvmrc` dado e o
    `package.json` no diretório dele), ao lado de `in_directory`
- [ ] **Passo 1: Escrever os testes que falham** — `first_allowed_ignores_lts`,
  `in_major_keeps_only_that_major`, `majors_are_ascending_and_distinct`,
  `beside_puts_package_json_next_to_the_nvmrc`,
  `the_restore_failure_line_names_the_command`.
- [ ] **Passo 2: Rodar** `cargo test project` — esperado: falha.
- [ ] **Passo 3: Implementar** a movimentação e os acréscimos; saída do init
  inalterada.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa (todos os testes do
  init verdes).
- [ ] **Passo 5: Commit** — `refactor(project): share version selection and
  file writing`.

### Tarefa 2: Argumentos do `upgrade`

**Arquivos:**

- Criar: `src/commands/upgrade/mod.rs`, `src/commands/upgrade/request.rs`
  (com testes)
- Modificar: `src/commands/mod.rs`, `src/cli/commands.rs` (um `Command::Upgrade
  { args }` roteado como o `Init`, com um comentário de documentação de ajuda)

**Interfaces:**

- Produz:
  - `enum Major { Same, Any, Exactly(u64) }`
  - `struct UpgradeRequest { pub tools: Vec<Tool>, pub major: Major }`
    (`tools` vazio = toda ferramenta fixada; ordem node, npm, yarn, pnpm)
  - `fn parse(args: &[String]) -> Result<UpgradeRequest, CliError>`: flags
    `--node`, `--npm`, `--yarn`, `--pnpm`, `--major`, `--major=<N>` (dígitos);
    uma flag repetida, `--major=` sem dígitos ou qualquer outro argumento é
    `CliError::InvalidOptions` (saída 6): `nvm upgrade: --npm is given twice.`,
    `nvm upgrade: --major=x is not a major version.`, `nvm upgrade:
    unsupported argument "x".`; `--major=<N>` com duas ou mais flags de
    ferramenta é saída 6: `nvm upgrade: --major=<N> applies to one tool; name
    exactly one.`
  - `pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output,
    CliError>` — até a Tarefa 4 devolve `CliError::Unsupported("nvm upgrade is
    not available yet.")` depois de interpretar
- [ ] **Passo 1: Escrever os testes que falham** —
  `no_flags_select_every_pinned_tool`,
  `tool_flags_select_those_tools_in_order`, `major_alone_means_any`,
  `major_with_a_number_means_that_major`, `a_repeated_flag_is_a_usage_error`,
  `a_major_that_is_not_a_number_is_a_usage_error`,
  `major_n_with_two_tools_is_a_usage_error`,
  `an_unknown_argument_is_a_usage_error`; um teste da CLI de que `nvm upgrade
  --help` lista as flags.
- [ ] **Passo 2: Rodar** `cargo test upgrade` — esperado: falha.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(upgrade): parse the upgrade flags`.

### Tarefa 3: Um alvo por ferramenta

**Arquivos:**

- Criar: `src/commands/upgrade/target.rs` (+ `target_tests.rs`)

**Interfaces:**

- Consome: `node_candidates`, `tool_candidates`, `first_allowed`, `in_major`,
  `majors` da Tarefa 1; `Judge`.
- Produz:
  - `enum Target { UpToDate, Move(Version), NoneInMajor { major: u64,
    newest: Option<(Version, Vec<Violation>)>, lowest: Option<u64> } }`
  - `fn target(context: &Context<'_>, judge: &mut Judge, tool: Tool, current:
    &Version, major: Major) -> Result<Target, PolicyError>`: candidatos da
    ferramenta (yarn: o pacote de `current` para `Major::Same`, da major `N`
    para `Exactly(N)`, `@yarnpkg/cli-dist` para `Any`), filtrados por major
    (`Same` → `current.major`), primeiro permitido; `UpToDate` quando é igual
    a `current`; `NoneInMajor` com o candidato mais novo nessa major e suas
    violações, e a menor major (entre todos os candidatos da ferramenta) que
    tem uma versão permitida
- [ ] **Passo 1: Escrever os testes que falham** —
  `moves_to_the_newest_allowed_in_the_same_major` (`20.11.1` com `20.18.0` e
  `22.11.0` permitidos → `Move(20.18.0)`), `major_any_crosses_majors` (→
  `22.11.0`), `major_n_picks_that_major`, `already_newest_is_up_to_date`,
  `a_denied_current_version_moves_to_the_newest_allowed`,
  `no_allowed_release_in_the_major_names_the_newest_and_the_lowest_major`,
  `yarn_one_stays_on_yarn_and_major_moves_to_cli_dist` (URLs das requisições),
  `node_without_lts_only_may_pick_a_non_lts_release`.
- [ ] **Passo 2: Rodar** `cargo test target` — esperado: falha.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(upgrade): pick the newest allowed release in
  the major`.

### Tarefa 4: Reescrever os arquivos do projeto

**Arquivos:**

- Criar: `src/commands/upgrade/project.rs` (+ `project_tests.rs`)
- Modificar: `src/commands/upgrade/mod.rs`, `src/commands/policy/report.rs` (um
  bloco para uma versão vinda de um `.nvmrc`, ao lado de `argument_block`)

**Interfaces:**

- Consome: Tarefas 1–3; `find_nvmrc`, `process_content`, `VersionPattern`,
  `parse_pins`, o mapeamento do erro de fixação do Plano 13.
- Produz: `fn upgrade(context: &Context<'_>, request: &UpgradeRequest) ->
  Result<Upgraded, CliError>` com `enum Upgraded { Nothing, Written {
  nvmrc: PathBuf } }` e sua saída num `Transcript` que lhe é dado:
  1. `.nvmrc` mais próximo (nenhum → saída 3), versões atuais (regras da linha
     de node e erros de fixação das Restrições globais, saída 3), a seleção
     (uma flag para uma ferramenta não fixada → saída 6; `--major=<N>` com uma
     seleção que não é exatamente uma ferramenta → saída 6) — tudo antes de
     qualquer requisição
  2. `Judge::load`, depois um alvo por ferramenta selecionada; qualquer
     `NoneInMajor` → as mensagens das Restrições globais para cada uma dessas
     ferramentas, saída 7, nada escrito
  3. mudanças impressas, ferramentas em dia informadas; nenhuma mudança →
     `Nothing`
  4. novo texto do `.nvmrc` e edição do `package.json` (yarn/pnpm mudaram)
     calculados, e então `save(context, "nvm upgrade",
     ProjectFiles::beside(nvmrc), ..)`
  - `run` imprime o transcript; a instalação vem na Tarefa 5

- [ ] **Passo 1: Escrever os testes que falham** —
  `upgrades_every_pinned_tool_by_default` (`.nvmrc` `v20.11.1` + `pnpm=9.1.0`,
  `package.json` `pnpm@9.1.0` → `v20.18.0`, `pnpm=9.12.0`, `pnpm@9.12.0`, as
  duas linhas de mudança), `a_tool_flag_upgrades_only_that_tool`,
  `a_flag_for_an_unpinned_tool_is_a_usage_error`,
  `a_partial_node_line_is_upgraded_within_its_major`,
  `an_alias_node_line_is_refused`, `no_nvmrc_is_refused`,
  `one_tool_without_a_target_writes_nothing` (saída 7, o bloco nomeia a regra
  e sua origem, a dica `--major=`), `everything_up_to_date_writes_nothing`,
  `the_nvmrc_in_a_parent_directory_is_the_one_rewritten`,
  `major_n_with_two_pinned_tools_and_no_flag_is_a_usage_error`.
- [ ] **Passo 2: Rodar** `cargo test upgrade` — esperado: falha.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(upgrade): rewrite .nvmrc to the newest
  allowed versions`.

### Tarefa 5: Instalar depois de escrever

**Arquivos:**

- Modificar: `src/commands/upgrade/mod.rs`,
  `src/commands/upgrade/project_tests.rs` (ou um novo `install_tests.rs`)

**Interfaces:**

- Consome: o `Upgraded` da Tarefa 4; `crate::commands::install::run(context,
  &[])`.
- Produz: `run` → depois de `Upgraded::Written`, `install::run(context, &[])`;
  seu stdout e stderr seguem as linhas do upgrade; seu status é o status do
  comando; num status que não é de sucesso, acrescentar `nvm upgrade:
  /p/.nvmrc is updated, but the install failed; run nvm install to finish.`;
  `Upgraded::Nothing` não instala nada.
- [ ] **Passo 1: Escrever os testes que falham** —
  `an_upgrade_installs_the_new_versions` (espelho, registro e arquivo
  simulados: node e pnpm terminam instalados),
  `a_failed_install_keeps_the_files_and_says_so`,
  `nothing_to_upgrade_installs_nothing` (nenhuma requisição depois dos alvos).
- [ ] **Passo 2: Rodar** `cargo test upgrade` — esperado: falha.
- [ ] **Passo 3: Implementar**.
- [ ] **Passo 4: Rodar** `cargo test` — esperado: passa.
- [ ] **Passo 5: Commit** — `feat(upgrade): install the upgraded versions`.

### Tarefa 6: Documentação

**Arquivos:**

- Modificar: `README.md`, `docs/deviations.md`
- [ ] **Passo 1: README** — uma seção "Upgrading a project": seleção, padrão na
  mesma major, `--major`, `--major=<N>`, o node pode escolher uma versão que
  não é LTS a menos que `ltsOnly`, linhas de node parciais viram exatas,
  aliases recusados, a mensagem de nenhum alvo e a dica, as escritas e o
  install, códigos de saída.
- [ ] **Passo 2: deviations.md** — o nvm.sh não tem `upgrade`.
- [ ] **Passo 3: Verificar** — `rumdl fmt` + `rumdl check` nos dois arquivos,
  `cargo test`.
- [ ] **Passo 4: Commit** — `docs: describe nvm upgrade`.
