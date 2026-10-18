# Desvios do nvm.sh

[English](deviations.md) | Português (Brasil)

Cada item é fixado por um teste; os cenários do contrato que fixam um deles
citam o seu id.

## O contrato (D1 a D17)

- D1: quando stdout e stderr vão para o mesmo lugar (`2>&1`), o nvmrc escreve
  todo o stderr primeiro; o nvm.sh intercala (`Found '.nvmrc'` primeiro). Com
  fluxos separados a saída é idêntica. (Cenários N7, X7.)
- D2: `--help` imprime a ajuda do clap, não o texto do nvm.
- D3: corrigido no Plano 9: um `nvm` sem argumentos imprime a ajuda com status
  0.
- D4: `--version` imprime `nvm <nvmrc version>`, não o número de versão do
  nvm.
- D5: não existe `nvm unload` (status 127); remova a linha de init no lugar.
  (S8)
- D6: um comando desconhecido recebe o erro do clap, não o despejo de ajuda
  do nvm; o status é 127 nos dois. (S9)
- D7: corrigido no Plano 9: `nvm alias` cria `$NVM_DIR/alias/lts`; uma falha
  ao criá-lo é silenciosa (o `mkdir` do nvm.sh reclama e segue em frente).
- D8: corrigido no Plano 9: `nvm ls v0.1.` lista v0.1.x.
- D9: `ls-remote` sem nada para listar imprime `N/A` sem a estrela que o
  nvm.sh imprime depois dele. (I5, I11)
- D10: `install -b` de uma versão anterior à v0.12 diz que o layout legado não
  é suportado (nvm.sh: "Binary download is not available"); status 3 nos
  dois.
- D11: `set-colors` com uma configuração inválida imprime a mensagem sem o
  despejo de ajuda. (S10)
- D12: corrigido no Plano 9: o node do sistema é o que `nvm deactivate` deixa
  no `PATH` (`$NVM_DIR/../sys` incluído).
- D13: uma versão v0.x colocada em `versions/node` (o nvm a coloca no próprio
  `$NVM_DIR`) é listada por `ls` e `alias`, mas não é usada por `use`.
- D14: não existe `nvm debug`, então o exit 42 nunca é produzido.
- D15: um comando que `exec` ou `run` não consegue iniciar imprime
  `nvm: <cmd>: not found` (`nvm-exec: ...` para o nvm-exec), não a mensagem
  de `exec` do bash; status 127 nos dois. (X6)
- D16: corrigido no Plano 9: as mensagens de piso nomeiam o caminho real de
  `min-version`.
- D17: quando `$NVM_DIR` termina com `/`, o nvm.sh avisa no stderr que ele não
  deveria ter barras finais; o nvmrc remove a barra e não imprime nada. (S7)

Mais duas diferenças são visíveis nos cenários sem um id próprio:

- Cenário L12: com um `versions/node` vazio, `nvm ls` no nvm.sh imprime
  `find: .../versions/node/*: No such file...` no stderr; o nvmrc não imprime
  nada. O cenário oculta o stderr, como o próprio teste do nvm.
- `nvm exec` e `nvm run` esperam o seu comando como processo filho, então o
  status dele é repassado depois que termina; só o `nvm-exec` se substitui
  pelo comando (no Unix).

## Por área

### Listagem

- `ls` não mostra a linha em branco fantasma quando há só um node do sistema.
- io.js e node que compartilham um número são ambos listados.
- `ls unstable` imprime uma única linha `N/A`.
- As versões são ordenadas por bytes.
- `ls` não lê `.nvmrc` como padrão e não enxerga um io.js do sistema.
- `unalias` não cria `alias/`.
- Um nome de alias inválido sai com 1.
- Caminhos que não são UTF-8 são mostrados com perda.

### Remoto

- O ureq substitui curl e wget; só o `NVM_AUTH_HEADER` é repassado.
- O índice é buscado uma única vez.
- `version-remote --lts=<bad>` imprime uma mensagem e `N/A`, status 3.
- Os padrões são texto literal, não regexes.
- As linhas são ordenadas por versão.
- Um mirror recusado é reportado uma única vez.

### Instalação

- O tipo de arquivo segue a versão e o SO; não há fallback para `.tar.gz` nem
  `xz` externo.
- Não há barra de progresso nem linha `Computing checksum`.
- Os arquivos são lidos na memória até 1 GiB; o xz usa um dicionário de 256
  MiB.
- Uma divergência de checksum falha e o arquivo é removido.
- Não há bootstrap do npm para um node sem npm.
- `--default` e `--alias` com `--lts` criam o alias `lts/<name>`.
- `--save` escreve `$PWD/.nvmrc` depois de uma instalação nova.
- Nenhuma instalação anterior à v0.12, e nenhuma anterior à v0.8.6 ou sem
  binário e fonte: status 3. Uma plataforma sem builds a partir do código
  fonte diz isso, status 1.
- `uninstall` não faz checagem de permissão e remove aliases por
  correspondência literal.
- Os jobs de build vêm da contagem de CPUs; a saída dos comandos é mostrada no
  fim.
- Só as instalações de binário são um único rename atômico.
- Os timeouts de conexão e de cabeçalho são de 30 s.

### Shell

- `use` de uma versão resolvível mas ausente é status 3 (nvm.sh: 0).
- `-h` e `help` são argumentos comuns.
- `deactivate` com um `NVM_DIR` não resolvível falha com 1.
- Um comando que não pode ser executado é 127; o bash diz 126.
- A função precisa de `nvm` no `PATH` e executa o primeiro que encontrar lá,
  inclusive um pacote npm global chamado `nvm`.
- `install` ativa só por meio da função.
- Só bash, zsh, sh, dash, ksh e fish são suportados.
- `--delete-prefix` sem npm avisa.
- `hash -r` é sempre emitido.
- Argumentos que não são UTF-8 são substituídos com perda.
- A função precisa de `/dev/fd`.

### Política de segurança

Adições do nvmrc sem equivalente no nvm.sh; sem uma política, uma flag de
política ou `NVMRC_POLICY`, nada muda (fixado por
`legacy_mode_without_any_new_configuration` em
`src/commands/policy/tests/mode.rs`).

- `--policy` e `--min-node` são lidas antes do subcomando ou em qualquer
  lugar depois dele, até um `--`. Depois de `exec` e `run` elas são lidas só
  logo após o subcomando; uma ocorrência posterior ali pertence ao programa e
  é deixada como está. Um valor vazio ou em branco é `nvm: <flag> needs a
  value.`, status 127.
  (`flags_before_the_subcommand_are_taken`,
  `flags_anywhere_after_another_subcommand_are_taken`,
  `flags_after_a_double_dash_are_left_alone`,
  `flags_after_the_first_argument_of_exec_or_run_are_left_alone`,
  `an_empty_or_blank_value_is_an_error` em
  `src/domain/policy/flags_tests.rs`; `a_flag_error_is_a_usage_error`,
  `a_policy_flag_after_the_version_of_use_is_enforced`,
  `a_floor_flag_after_the_version_of_install_is_enforced`,
  `a_policy_flag_after_the_program_of_exec_reaches_the_program` em
  `src/cli/tests/policy.rs`)
- `--policy` substitui `NVMRC_POLICY`; as duas nunca são combinadas.
  (`a_policy_flag_wins_over_the_policy_variable`)
- Uma versão ilegível do node do sistema sob uma política é
  `nvm: cannot check the policy: the version of the system node cannot be
  read.`, status 9.
  (`exec_system_with_an_unreadable_version_says_so_and_fails_to_know`)
- `.nvmrc-policy.json` é procurado subindo a partir do diretório de trabalho.
  (`a_repository_policy_is_found_above_the_working_directory_and_enforces`)
- No modo política `install`, `use`, `exec` e `run` checam as regras do node;
  `__auto` avisa e segue em frente.
  (`install_below_a_policy_minimum_stops_before_downloading`,
  `use_of_a_disallowed_installed_version_stops_without_switching`,
  `exec_with_a_disallowed_version_runs_nothing`,
  `run_goes_through_the_same_policy_gate`,
  `auto_install_of_a_disallowed_version_warns_and_goes_on`,
  `auto_use_with_an_unreadable_policy_warns_and_switches`)
- A mensagem de violação nomeia a versão, a sua origem e cada regra com a sua
  fonte. (`a_violation_stops_with_7_and_names_everything`,
  `run_from_an_nvmrc_names_the_file_in_the_policy_message`)
- As fontes só apertam; uma mais frouxa avisa.
  (`the_highest_minimum_wins_and_names_its_source`,
  `a_lower_override_loses_with_a_warning`)
- Exit 9 significa que a política não pôde ser verificada, e exit 7 também
  cobre uma versão que a política não permite.
  (`an_invalid_central_policy_is_a_failure_to_know`,
  `an_unreachable_index_is_a_failure_to_know`,
  `offline_with_a_release_rule_is_a_failure_to_know`,
  `a_min_version_file_that_cannot_be_read_is_an_error`)
- `nvm-exec` mantém o status de uma recusa da política (7, ou 9 quando a
  política não pode ser verificada); suas outras falhas de seleção continuam
  127.
  (`a_policy_refusal_keeps_its_own_status` em
  `src/commands/nvm_exec/tests.rs`)
- `use system` é verificado só no modo política.
  (`use_system_without_a_policy_never_asks_the_system_node`)
- Locais de política `http://` são rejeitados, e também um redirecionamento de
  um local `https://` para http. (`an_http_location_is_rejected`,
  `a_policy_fetch_refuses_a_redirect_away_from_https`,
  `a_secure_fetch_never_contacts_a_url_that_is_not_https`)
- Nos caminhos automáticos a política e o índice são buscados uma única vez,
  com limite de 5 segundos, em vez de três tentativas de até 30 segundos.
  (`an_advisory_policy_fetch_goes_through_the_quick_client_once`,
  `an_advisory_index_fetch_goes_through_the_quick_client`)
- A função de shell pula as flags de política iniciais para achar o
  subcomando.
  (`every_function_routes_on_the_first_argument_that_is_not_a_policy_flag`,
  `use_after_leading_policy_flags_switches_the_shell` em
  `tests/init_policy_cli.rs`)
- A checagem de CVE lê o índice do security-wg do Node.js: afetado significa
  estar em `vulnerable` e não em `patched`, filtrado por
  `affectedEnvironments`; uma severidade ausente ou `unknown` é `high`,
  qualquer outro valor desconhecido é uma falha em saber; um aviso sem ids de
  CVE é `advisory <id>`; io.js é ignorado.
  (`a_vulnerable_unpatched_version_is_affected`,
  `a_patched_version_is_not_affected`,
  `a_platform_specific_advisory_counts_only_on_its_platform`,
  `an_advisory_without_cve_ids_is_named_by_id`,
  `unknown_and_missing_severities_count_as_high`,
  `anything_else_is_a_failure_to_know` em
  `src/domain/policy/advisories_tests.rs`;
  `iojs_versions_skip_the_cve_check` em
  `src/commands/policy/tests/advisories.rs`)
- `--fail-on-severity`, `NVMRC_FAIL_ON_SEVERITY` e `cve.failOnSeverity`
  apertam uns aos outros (padrão `high`, `none` o mais frouxo, um mais frouxo
  avisa); a variável liga o modo política.
  (`without_any_setting_the_limit_is_high_and_the_index_is_the_default`,
  `the_strictest_limit_wins_and_a_looser_override_warns`,
  `none_is_the_loosest_limit` em `src/domain/policy/cve_tests.rs`;
  `the_severity_flag_and_variable_turn_policy_mode_on`,
  `the_severity_flag_can_only_tighten` em
  `src/commands/policy/tests/mode.rs`)
- O índice é o `cve.source` da política central, senão o padrão, mesmo quando
  o repositório nomeia um; o do repositório conta só sem uma política
  central. Uma fonte relativa é lida ao lado do seu arquivo de política e é
  inválida numa política https.
  (`the_central_index_wins_and_the_repository_index_counts_only_alone`
  em `src/domain/policy/cve_tests.rs`;
  `a_relative_index_location_is_read_next_to_its_policy`,
  `a_relative_index_in_a_central_file_is_read_next_to_it`,
  `a_relative_index_in_a_remote_policy_is_invalid` em
  `src/commands/policy/tests/mode.rs`)
- Um achado no limite ou acima dele é status 7; abaixo, apenas um aviso (todos
  os achados com `none`). (`a_finding_at_the_limit_stops_with_7`,
  `findings_below_the_limit_only_warn`, `limit_none_reports_and_passes`,
  `a_patched_version_passes_quietly`,
  `findings_below_the_limit_are_also_named_when_the_gate_stops` em
  `src/commands/policy/tests/advisories.rs`)
- Um índice ilegível ou inválido é status 9, ou um aviso quando as regras do
  node já param com 7. `--offline` com um índice https é 9 antes de qualquer
  requisição; um índice local funciona offline. Os caminhos automáticos
  avisam e passam; o modo legado nunca busca.
  (`an_unreachable_index_is_a_failure_to_know`,
  `an_invalid_index_names_its_location`,
  `offline_with_a_remote_index_is_a_failure_to_know`,
  `offline_with_a_local_index_is_checked`,
  `an_automatic_path_warns_on_a_finding_and_passes`,
  `legacy_mode_never_fetches_the_index` em
  `src/commands/policy/tests/advisories.rs`;
  `a_node_violation_still_stops_with_7_when_the_index_is_unreachable`,
  `a_node_violation_still_stops_with_7_offline_with_a_remote_index` em
  `src/commands/policy/tests/unknown_index.rs`)
- `use --save` no modo política escreve o `.nvmrc` só depois que a política
  permite a versão; a ordem legada não muda.
  (`a_refused_use_save_leaves_nvmrc_untouched`,
  `an_allowed_use_save_still_writes_nvmrc` em
  `src/commands/use_version/tests.rs`)

### npm, yarn e pnpm

Adições do nvmrc; o nvm.sh ignora as linhas `npm=`, `yarn=` e `pnpm=` no
`.nvmrc`.

- As linhas mudam o que `install`, `use`, `exec`, `run` e `deactivate` fazem:
  `install` busca as ferramentas fixadas; `use`, `exec` e `run` as colocam no
  `PATH` e descartam as entradas de outros projetos; `deactivate` as remove.
- Um erro de pin, um `packageManager` no `package.json` vizinho que discorda
  do pin, ou uma ferramenta fixada que não está instalada sai com 3 num `use`,
  `exec` ou `run` explícito; os caminhos automáticos só avisam.
- As ferramentas ficam em `$NVM_DIR/package-managers/<tool>/<version>/`,
  compartilhadas por todas as versões do node, e os seus diretórios `bin`
  lideram o `PATH`.
- `NVM_AUTH_HEADER` não é enviado ao registro do npm, aos hosts de tarballs
  nem ao índice de vulnerabilidades embutido; `NVMRC_NPM_AUTH_HEADER` vai só
  ao host do registro.
- `install --save` e `use --save` (`-w`) substituem apenas a linha da versão
  simples do `.nvmrc`, mantendo o seu comentário final, e mantêm todas as
  outras linhas (pins, outras chaves, comentários); o nvm.sh reescreve o
  arquivo inteiro. Um arquivo sem mais nada é escrito só como `<version>`
  (com esse comentário, se houver), como faz o nvm.sh, e um `.nvmrc` que não
  pode ser lido é deixado como está (status 3).
  (`saving_keeps_the_pins_and_the_other_lines`,
  `saving_over_a_lone_version_writes_just_the_version`,
  `an_unreadable_nvmrc_is_left_alone` em `src/commands/nvmrc_file.rs`)
- Um `.nvmrc` que não pode ser lido gera
  `nvm: warning: <path> cannot be read: <reason>` e nenhum pin; o npm
  embutido é então verificado quando existe uma regra de npm.
  (`an_unreadable_nvmrc_is_a_warning_and_no_pins` em
  `src/commands/project_pins.rs`)

### Iniciando um projeto

O nvm.sh não tem modo de projeto para `init`; isto é uma adição do nvmrc.

- `nvm init` com `--node`, `--npm`, `--yarn` ou `--pnpm` e sem shell escreve
  a release permitida mais nova (LTS para o node), ou a versão exata dada, no
  `.nvmrc` do diretório de trabalho, e define `packageManager` num
  `package.json` existente para yarn ou pnpm. `nvm init <shell>` não muda.
- Misturar um nome de shell com uma flag de projeto, `--yarn` com `--pnpm`,
  uma flag repetida, ou `--no-use`/`--install` com uma flag de projeto sai com
  6.
- Exit 3 para uma versão inexata ou não publicada, um conflito de pin (yarn
  sobre pnpm ou o inverso) e um pin ou `package.json` malformado; 7 quando não
  permitido; 9 quando a resposta não pode ser conhecida. Nada é escrito numa
  recusa. Um arquivo que não pode ser lido ou escrito sai com 1; quando a
  escrita do `.nvmrc` falha depois da do `package.json`, o `package.json` é
  restaurado.

### Atualizando um projeto

O nvm.sh não tem `upgrade` para um projeto; isto é uma adição do nvmrc. (`nvm
upgrade` não é o `nvm install-latest-npm` do nvm.sh, que atualiza o npm do
node em uso.)

- `nvm upgrade` move as ferramentas fixadas no `.nvmrc` mais próximo para a
  release permitida mais nova no seu major atual (`--major`: qualquer major;
  `--major=<N>`: major N, para exatamente uma ferramenta), reescreve o
  `.nvmrc` e o `packageManager`, e então executa `install` sem argumento.
- Uma linha de alias do node é recusada (exit 3) só quando o node é
  selecionado.
- Exit 6 para um erro de uso, 3 para ausência de `.nvmrc`, uma linha de alias
  ou um pin inválido, 7 quando não existe release permitida no major (nada é
  escrito), 9 quando a resposta não pode ser conhecida. Quando a instalação
  falha depois da escrita, o seu status ou o código do erro é mantido e uma
  linha diz para executar `nvm install`.

### Cores

- Uma letra inválida em `NVM_COLORS` avisa uma vez; um `TERM` desconhecido é
  silencioso.
- Não há legenda de cores em `--help`.
- `NVM_NO_COLORS` não vaza.
- Linhas inválidas do `.nvmrc` e destinos de alias são impressos crus,
  inclusive os bytes de controle.
- `NVM_COLORS` é lido por caractere.
- A linha de `--alias`/`--default` da instalação nunca é colorida.
- `set-colors x --help` define as cores.

### Conflitos

- Não há ripgrep nem `LineScanner`.
- A checagem em tempo de execução está no snippet de init, então um `source
  nvm.sh` tardio só é encontrado pelo `doctor`.
- Arquivos de lazy-loader nunca são editados.
- `OmzPlugin` e `HelperCall` são dicas.
- `source` precisa estar em posição de comando; um `zsh-defer` ou `time`
  antes dele conta; `eval "$(cat nvm.sh)"` não é visto.
- Here-documents não são rastreados.
- Caminhos relativos de source são resolvidos a partir do diretório do
  arquivo que faz o source.
- Só as linhas de loader e de completion são migradas; linhas compostas são
  manuais, assim como uma linha dentro de um bloco ou continuada.
- Um marcador BEGIN sem fim não é um bloco.
- A linha de carga chama `nvmrc`.
- O shell do bloco vem do nome do arquivo.
- O diff faz parte do prompt.
- Os backups mantêm as permissões do arquivo, mas não o dono, os atributos
  estendidos nem as ACLs, e um profile somente leitura é substituído.
- `--undo` faz backup primeiro, recusa um backup vazio e prefere backups sem
  data no futuro.
- Um arquivo ilegível é exit 1.
- Um arquivo que o verificador de sintaxe já recusa é deixado como está; a
  ausência do verificador escreve com um aviso.
- O fish é verificado na imagem Docker e no CI (instalado pelo apt).
- `doctor`, `migrate` e `__conflict` são adições.

## Plataformas

A v1 roda em Linux e macOS (x86_64 e arm64). O Windows não é suportado: o
canal do shell é um descritor de arquivo herdado, o `nvm-exec` se substitui
com `exec`, e as instalações definem permissões Unix, então compilar para
Windows para com um erro de compilação que nomeia este arquivo. Não tratados:

- O AIX recebe `.tar.xz` (o nodejs.org publica `.tar.gz` lá).
- armv6 e Rosetta não são detectados como o nvm.sh faz.
- Um lock de instalação deixado por Ctrl-C espera o seu timeout de lock
  obsoleto.
- O `.nvmrc` é procurado a partir de `$PWD` como o shell o fornece.
