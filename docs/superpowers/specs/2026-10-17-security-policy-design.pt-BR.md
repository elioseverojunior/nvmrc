# Política de segurança, regras de versão e gerenciadores de pacotes

Data: 2026-10-17
Status: Design, ainda sem plano
Baseado em: [o design do nvmrc](2026-10-02-nvmrc-design.md) e
[deviations.md](../../deviations.md)

## 1. Intenção

Em pipelines enterprise, quem decide quais versões dos SDKs de desenvolvimento
são permitidas é o time de segurança, não o time do projeto. Hoje nada num
pipeline impede um projeto cujo `.nvmrc` pede um Node, npm, yarn ou pnpm
antigo, vulnerável ou ainda não avaliado.

O nvmrc passa a ser o lugar que faz essa decisão valer. Times e empresas que
adotam o nvmrc como toolset padrão publicam um arquivo de política num formato
definido pelo nvmrc. A partir daí, todo pipeline que roda o nvmrc aplica a
política. Um projeto numa versão que a política não permite falha o build com
um erro que diz exatamente o que está errado e como corrigir, e o desenvolvedor
corrige com um único comando explícito.

O nvmrc também passa a instalar e fixar npm, yarn e pnpm ao lado do Node, de
modo que o mesmo arquivo (`.nvmrc`) descreve todo o toolchain e a mesma
política cobre tudo.

O formato que o nvmrc define é um contrato sobre o qual outras empresas
constroem. Ele precisa ser rígido e sem ambiguidade: um erro de digitação numa
política nunca pode virar "tudo permitido", e nada que o time do projeto
configure pode enfraquecer o que o time de segurança decidiu.

## 2. Objetivos

1. Um formato de **arquivo de política** versionado (JSON) com um **JSON
   Schema** publicado, carregável de um caminho local ou de uma URL, e que
   também pode ser commitado no repositório do projeto como
   `.nvmrc-policy.json`.
2. **Regras de versão** por ferramenta para **node, npm, yarn e pnpm**: um
   mínimo, uma faixa permitida, uma lista de proibidas, só LTS (node) e uma
   idade mínima de release. Flags e variáveis só podem apertar a política.
3. Uma **checagem de CVE** para a versão do Node em uso, que falha no limiar de
   severidade definido pela política ou acima (padrão `high`).
4. **Erros claros**: a versão pedida e sua origem, a regra que ela viola e a
   origem dessa regra, os CVEs com sua severidade, e a correção.
5. **`nvm init --node|--npm|--yarn|--pnpm`** gera o `.nvmrc` de um projeto (e o
   `packageManager` no `package.json`) já de acordo com a política.
6. **`nvm upgrade`** leva um projeto existente às versões permitidas mais novas,
   dentro da major atual, a menos que o desenvolvedor peça para cruzá-la.
7. O nvmrc **instala e fixa npm, yarn e pnpm**, registrados no `.nvmrc`, com
   yarn e pnpm espelhados no `packageManager` do `package.json`.

## 3. Fora do escopo

- Mudar as versões de um projeto sem que peçam. A aplicação da política só
  reporta; `init` e `upgrade` só mudam arquivos quando uma pessoa os roda.
- Checagem de CVE para npm, yarn e pnpm. As regras de versão deles são
  aplicadas; os CVEs deles ficam fora deste design.
- Cache da política, dos dados de vulnerabilidade ou dos metadados do registro
  entre execuções (o nvmrc não faz cache de nenhum índice remoto hoje; ver
  deviations.md).
- Ler `.node-version`, `engines`, `volta` ou `devEngines`.
- Usar o corepack. Ele deixou de vir junto com o Node a partir da versão 25,
  então depender dele faria o nvmrc se comportar diferente conforme a versão do
  Node.

## 4. Antiobjetivos

Qualquer um destes torna a funcionalidade um fracasso, mesmo que ela funcione no
caminho feliz:

- Num comando explícito (`install`, `use`, `exec`, `run`, `init`, `upgrade`),
  uma política que não pode ser lida, interpretada ou validada, ou uma fonte de
  CVE ou registro inacessível, deixa o comando passar. **Toda falha em saber é
  uma falha.**
- Um campo desconhecido numa política é ignorado em vez de rejeitado.
- Uma flag, variável de ambiente ou política do repositório enfraquece uma regra
  definida pela política central.
- O `upgrade` cruza uma major sem que peçam.
- As flags quebram a função de shell: `nvm --policy p use 20` precisa continuar
  trocando o Node do shell.
- O comportamento sem a nova configuração muda. Sem política e sem nenhuma das
  novas flags ou variáveis, o nvmrc se comporta exatamente como hoje (incluindo
  `NVM_MIN_VERSION` e `$NVM_DIR/min-version`), e a suíte de compatibilidade com
  o nvm.sh passa sem mudanças.
- Abrir um shell ou trocar de diretório numa máquina de desenvolvedor falha por
  causa da política. Caminhos automáticos avisam; nunca bloqueiam.
- `nvm init bash` (o snippet de shell existente) muda de comportamento.

## 5. Restrições do código existente

- Os comandos interpretam os próprios argumentos à mão (`args: Vec<String>`,
  como o nvm.sh); o clap só roteia os subcomandos. Variáveis de ambiente são
  lidas pelo port `Env` para que os testes possam simulá-las, então **a feature
  `env` do clap não é usada**.
- A função de shell roteia pelo `$1` (`src/shell/init/posix.rs:16-26`,
  `src/shell/init/fish.rs:26`); só `use`, `deactivate`, `install`, `i`,
  `set-colors` e `__auto` recebem o canal que aplica código no shell.
- Já existe um piso: `VersionFloor` (`src/domain/floor.rs`), vindo de
  `NVM_MIN_VERSION` com fallback para `$NVM_DIR/min-version`, checado só no
  `install`, código de saída 7. Este design o estende; não cria um segundo
  mecanismo de mínimo.
- `nvm init <shell> [--no-use|--install]` existe (`src/commands/init.rs`) e hoje
  rejeita qualquer outra opção, então as novas flags não colidem. Não existe
  comando `upgrade`.
- `install --offline` existe.
- O parser do `.nvmrc` (`src/domain/nvmrc/mod.rs:66-96`) já aceita linhas
  `chave=valor` e ignora chaves diferentes de `node`, então linhas `npm=`,
  `yarn=` e `pnpm=` continuam legíveis pelo nvm.sh.
- O índice remoto é o `index.tab`. Ele traz a data e o nome LTS de cada release
  (hoje só as colunas de versão e LTS são lidas) e uma marcação de segurança,
  mas não tem severidade.
- Não há dependências de JSON. Crates novos precisam passar no `cargo deny`
  (só crates.io, allowlist de licenças no `deny.toml`). As regras do projeto
  continuam valendo: ports e adapters, fakes `&dyn`, sem rede nos testes,
  arquivos com menos de 300 linhas, funções com menos de 30, TDD, e todo desvio
  do nvm.sh fixado por um teste e listado no deviations.md.

## 6. Design

### 6.1 O arquivo de política

JSON, validado contra
`schemas/nvmrc-policy.v1.schema.json` (publicado neste repositório). Exemplo:

```json
{
  "$schema": "https://raw.githubusercontent.com/elioseverojunior/nvmrc/main/schemas/nvmrc-policy.v1.schema.json",
  "schemaVersion": 1,
  "tools": {
    "node": {
      "minimum": "20.11.1",
      "allow": ">=20 <25",
      "deny": ["20.12.0", "22.3.0"],
      "ltsOnly": true,
      "minimumReleaseAgeDays": 14
    },
    "npm": { "minimum": "10.2.4" },
    "pnpm": { "minimum": "9.1.0", "minimumReleaseAgeDays": 7 },
    "yarn": { "allow": ">=4.1.0 <5" }
  },
  "cve": {
    "failOnSeverity": "high",
    "source": "https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json"
  }
}
```

Regras:

- `schemaVersion` é obrigatório e precisa ser `1`. Qualquer outro valor é um
  erro que cita as versões que este nvmrc entende.
- Campos desconhecidos são rejeitados em todos os níveis.
- `tools` e cada ferramenta e campo dentro dele são opcionais; um campo ausente
  significa que aquela regra não restringe a ferramenta.
- Campos por ferramenta:
  - `minimum`: a mesma sintaxe que o `NVM_MIN_VERSION` aceita (`20`, `v20`,
    `20.11.1`); uma versão parcial significa sua release mais baixa (`20` é
    `20.0.0`).
  - `allow`: uma faixa semver no estilo npm (`>=20 <25`, `^20.11.1 || ^22`).
  - `deny`: uma lista de versões exatas.
  - `ltsOnly` (só node): `true` permite só releases que têm nome LTS no índice.
    Definir em outra ferramenta é um erro de validação.
  - `minimumReleaseAgeDays`: um inteiro não negativo; uma release só é
    permitida depois de publicada há pelo menos esse número de dias.
- `cve` é opcional. `failOnSeverity` é um de `critical`, `high`, `medium`,
  `low`, `none`, com padrão `high`. `none` desliga a falha, mas continua
  reportando. Com uma política configurada, a checagem de CVE roda mesmo sem o
  bloco `cve`.
- `cve.source` é opcional, uma URL ou um caminho local como a própria política.
  O padrão é o índice de vulnerabilidades do core do Node Security WG; o campo
  existe para que pipelines sem internet apontem para um mirror.
- `$schema` é permitido e ignorado, para que editores possam validar políticas.

Os tipos serde com `deny_unknown_fields` são quem faz valer as regras. O arquivo
de schema é o contrato publicado. Um teste gera o schema a partir dos tipos e
falha quando ele difere do arquivo commitado, de modo que há uma única fonte de
verdade (DRY).

### 6.2 Quando uma versão é permitida

Uma versão de uma ferramenta é **permitida** quando tudo isto vale:

1. ela está no `minimum` ou acima;
2. ela está dentro do `allow`;
3. ela não está no `deny`;
4. para o node com `ltsOnly`, ela tem nome LTS;
5. ela foi publicada há pelo menos `minimumReleaseAgeDays`;
6. para o node, ela não tem CVE no limiar de severidade ou acima (6.6).

As datas de release vêm do `index.tab` para o node e dos metadados `time` do
registro para npm, yarn e pnpm. A regra 5 precisa desses dados, então um índice
ou registro inacessível é uma falha em saber quando a regra está definida.

Uma única função decide isso, e a aplicação da política (6.5), o `init` (6.8) e
o `upgrade` (6.9) usam essa mesma função.

### 6.3 De onde vêm a política e as regras

Novas flags, cada uma com uma variável de ambiente:

- `--policy <caminho|url>`, `NVMRC_POLICY`.
- `--min-node <versão>`, `NVM_MIN_VERSION` (a variável existente; a flag
  alimenta o piso existente).
- `--min-npm`, `--min-yarn`, `--min-pnpm` com `NVMRC_MIN_NPM`,
  `NVMRC_MIN_YARN`, `NVMRC_MIN_PNPM`.
- `--fail-on-severity <nível>`, `NVMRC_FAIL_ON_SEVERITY`.

As outras regras (`allow`, `deny`, `ltsOnly`, `minimumReleaseAgeDays`) vêm só de
arquivos de política.

**A política do repositório.** Um projeto pode commitar um
`.nvmrc-policy.json`, com o mesmo schema da política central. O nvmrc o encontra
do mesmo jeito que encontra o `.nvmrc`: subindo a partir do diretório de
trabalho. Ele é mais uma fonte na combinação abaixo, então pode apertar regras,
mas nunca enfraquecer a política central. O `cve.source` dele só é usado quando
não há política central configurada; caso contrário, vence a fonte da política
central, para que um projeto não possa apontar a checagem para uma fonte
escolhida por ele.

**Overrides só apertam.** As fontes são: flag, variável de ambiente, política
central (`--policy` ou `NVMRC_POLICY`), política do repositório e
`$NVM_DIR/min-version` (só node). Cada regra é combinada entre todas as fontes
que a definem:

- `minimum`: vence o mais alto.
- `allow`: a interseção de todas as faixas (uma versão precisa atender a todas).
- `deny`: a união de todas as listas.
- `ltsOnly`: ligado se qualquer fonte ligar.
- `minimumReleaseAgeDays`: vence o maior.
- limiar de severidade: vence o mais rígido (`low` é mais rígido que `high`;
  `none` é o mais frouxo).

Uma fonte que tenta afrouxar uma regra não é um erro: o valor dela perde, e o
nvmrc imprime um aviso citando essa fonte e o valor que venceu. O erro de uma
violação cita a fonte da regra violada.

Um valor que começa com `http://` é rejeitado; `https://` é baixado pelo port
`Http` existente (com o tratamento de `NVM_AUTH_HEADER` de hoje); qualquer outra
coisa é um caminho local. Uma política que não pode ser baixada, lida,
interpretada ou validada é um erro, nunca "sem política".

### 6.4 Posição das flags e a função de shell

As flags são aceitas antes do subcomando (`nvm --policy p use 20`) e depois
dele (`nvm use --policy p 20`). Elas são removidas antes da interpretação de
argumentos do próprio subcomando, então nenhum comando as interpreta.

A função de shell gerada (POSIX e fish) pula as opções iniciais `--policy`,
`--min-*` e `--fail-on-severity`, nas formas `--flag valor` e `--flag=valor`,
antes de decidir o subcomando, para que `use`, `install` e os demais continuem
recebendo o canal do shell. Nos pipelines, a interface esperada são as
variáveis de ambiente; as flags servem para overrides.

### 6.5 Quando e onde é aplicada

**Modo legado.** Quando só `NVM_MIN_VERSION` (como variável) ou
`$NVM_DIR/min-version` está definido, o nvmrc se comporta exatamente como hoje:
o piso do node é checado só no `install`, e não há checagem de CVE.

**Modo política** fica ligado quando uma política central está configurada, uma
política do repositório é encontrada, ou qualquer uma das novas flags ou
variáveis `NVMRC_*` está definida. A política do repositório sozinha basta,
para que um time possa se proteger antes de o time de segurança publicar uma
política central. `--min-node` conta como flag nova, mesmo alimentando o
`NVM_MIN_VERSION`. No modo política:

- Comandos explícitos checam se cada versão em vigor é permitida (6.2) e falham
  em qualquer violação ou falha em saber:
  - `install`: antes de qualquer download.
  - `use`, `exec`, `run`: na versão resolvida, porque um pipeline pode usar um
    Node que já estava instalado. `use system` checa a versão do Node do
    sistema.
  - `init` e `upgrade`: antes de gravar qualquer coisa.
- Caminhos automáticos só avisam: `__auto` (o hook de `cd`) e o `use` ou
  `install` que o snippet do `nvm init <shell>` roda ao abrir o shell. Uma
  violação ou uma falha em saber imprime um aviso, e o comando segue como
  seguiria sem política.
- Comandos que só reportam (`ls`, `ls-remote`, `current`, `which`, `version`,
  `version-remote`) não aplicam a política.

As versões em vigor são a do node e as de npm, yarn e pnpm descritas na 6.7. Os
pins de npm, yarn e pnpm são sempre lidos do `.nvmrc` mais próximo, mesmo quando
a versão do Node vem de um argumento na linha de comando.

`install --offline` no modo política exige uma política local, um `cve.source`
local e nenhuma regra `minimumReleaseAgeDays`; caso contrário, ele falha como
falha em saber.

### 6.6 Checagem de CVE

O formato do `cve.source` é o índice do core do Node Security WG
(`vuln/core/index.json`): entradas indexadas por id, com `cve`, `vulnerable`
(faixas semver no estilo npm), `patched`, `severity` e, quando presente,
`affectedEnvironments`. Um mirror precisa servir esse formato.

O nvmrc encontra as entradas cuja faixa `vulnerable` contém a versão do Node e
cujo `affectedEnvironments`, quando presente, inclui a plataforma atual. Ele
compara a severidade de cada entrada com o limiar:

- No limiar ou acima: a versão não é permitida (saída 7 em comandos
  explícitos).
- Abaixo do limiar: a versão é permitida, com um aviso listando os CVEs.
- Uma entrada sem severidade conta como `high`.
- Uma severidade fora dos cinco níveis, uma faixa que não pode ser
  interpretada, ou uma fonte inacessível ou ilegível é uma falha em saber.

### 6.7 npm, yarn e pnpm no `.nvmrc`

```text
20.11.1
npm=10.9.0
pnpm=9.12.0
```

- O `.nvmrc` é a fonte da verdade. `npm=`, `yarn=` e `pnpm=` recebem uma versão
  exata `major.minor.patch`, para que o `use` nunca precise da rede para
  resolvê-las. Um projeto fixa no máximo um entre yarn e pnpm; fixar os dois é
  um erro.
- Quando um projeto fixa yarn ou pnpm e tem um `package.json`, o
  `packageManager` dele precisa ser `<nome>@<versão>` com o mesmo nome e versão.
  Um sufixo de hash do corepack (`+sha512.…`) é permitido e ignorado na
  comparação. Um campo ausente ou um valor diferente falha com um erro que
  mostra os dois e cita o arquivo a corrigir; o nvmrc nunca reescreve o
  `package.json` fora do `init` e do `upgrade`.
- O `install` também instala as ferramentas fixadas. O nvmrc baixa o tarball do
  pacote do registro npm (`NVMRC_NPM_REGISTRY`, padrão
  `https://registry.npmjs.org`), verifica o hash de integridade do registro e
  instala em `$NVM_DIR/package-managers/<nome>/<versão>/`, compartilhado por
  todas as versões do Node. Pacotes: `npm` para o npm, `pnpm` para o pnpm,
  `yarn` para o yarn 1.x e `@yarnpkg/cli-dist` para o yarn 2 em diante.
- O `use` coloca os diretórios `bin` das versões fixadas no `PATH` antes do
  Node, de modo que um npm fixado tem precedência sobre o que vem com o Node.
  Dois projetos no mesmo Node podem fixar versões diferentes sem colidir.
- As regras valem para a versão em vigor: a fixada, ou, para o npm sem pin, o
  npm que vem com o Node em uso.

### 6.8 `nvm init` no modo projeto

- `nvm init <shell> [--no-use|--install]` não muda.
- `nvm init` com uma ou mais de `--node[=<versão>]`, `--npm[=<versão>]`,
  `--yarn[=<versão>]`, `--pnpm[=<versão>]` e sem nome de shell entra no modo
  projeto.
- Um nome de shell junto com uma flag de projeto é um erro de uso, assim como
  `--yarn` e `--pnpm` juntos.
- Sem versão, cada ferramenta recebe a release permitida mais nova (6.2); para
  o node, é o LTS permitido mais novo, com ou sem `ltsOnly` na política. Para o
  yarn, os candidatos vêm do `@yarnpkg/cli-dist`.
- Com versão, ela precisa ser exata e permitida, ou o comando falha antes de
  gravar.
- Ele grava o `.nvmrc` no diretório de trabalho, definindo só as chaves pedidas
  e mantendo o resto. Para yarn ou pnpm, ele define o `packageManager` num
  `package.json` existente.

### 6.9 `nvm upgrade`

`nvm upgrade [--node] [--npm] [--yarn] [--pnpm] [--major[=<N>]]`

- Sem flags de ferramenta, ele atualiza todas as ferramentas que o `.nvmrc` mais
  próximo fixa; com elas, só essas ferramentas. Uma flag para uma ferramenta
  que o `.nvmrc` não fixa é um erro (use o `init` para adicioná-la).
- **Padrão:** cada ferramenta vai para a release permitida mais nova (6.2)
  dentro da major que já usa. Ele nunca cruza uma major por conta própria.
- **`--major`:** vai para a release permitida mais nova em qualquer major, o que
  pode pular várias majors (de 20 para 25).
- **`--major=<N>`:** vai para a release permitida mais nova na major `N`. Com
  várias flags de ferramenta, `--major=<N>` é um erro de uso, a menos que
  exatamente uma ferramenta esteja selecionada.
- Quando a major atual não tem release permitida, o comando falha antes de
  gravar qualquer coisa, diz isso e sugere `--major` e a menor major que tem
  release permitida.
- Quando uma ferramenta já está na release permitida mais nova, ela fica como
  está e é reportada como atualizada.
- Ele reescreve o `.nvmrc` e, para yarn ou pnpm, o `packageManager` do
  `package.json`; depois instala as novas versões como o `install` faria e
  imprime cada mudança (`node 20.11.1 -> 20.18.0`). Se a instalação falhar, os
  arquivos já estão atualizados e o erro diz isso.
- Ele falha antes de gravar qualquer coisa se alguma ferramenta selecionada não
  tiver alvo permitido, para que os arquivos de um projeto nunca fiquem com o
  upgrade pela metade.

### 6.10 Erros e códigos de saída

- **7**: uma violação da política (uma versão não permitida, por qualquer regra
  da 6.2), como o piso hoje; também o `upgrade` quando não existe alvo
  permitido.
- **9** (novo, não usado pelo nvm.sh): uma falha em saber (política, fonte de
  CVE ou registro inacessível, ilegível, inválido ou `http://`; uma severidade
  desconhecida; offline com uma fonte remota). Pipelines conseguem distinguir
  "não permitido" de "não foi possível checar".

Toda mensagem de violação cita:

- a ferramenta e a versão pedida, e de onde ela veio (caminho do `.nvmrc`,
  argumento, `package.json`, junto com o Node);
- cada regra violada, e de onde essa regra veio (flag, nome da variável,
  caminho ou URL da política central, caminho da política do repositório,
  `min-version`);
- para CVEs, cada id de CVE com sua severidade;
- a correção, citando o comando quando existe um (`nvm upgrade --node`, ou
  `nvm upgrade --node --major` quando a major atual não tem release permitida).

Exemplo (a mensagem real é em inglês):

```text
nvm: node v18.19.0 (from /work/app/.nvmrc) is not allowed by policy
  https://sec.example.com/nvmrc-policy.json:
  - below the minimum 20.11.1
  - CVE-2024-27983 (high), CVE-2024-22019 (high)
No allowed release in major 18. Run `nvm upgrade --node --major`.
```

## 7. Ordem de entrega

Cada etapa pode ser lançada sozinha:

1. Arquivo de política e schema, flags e variáveis, a política do repositório,
   combinação que só aperta, modos legado e política, as regras do node exceto
   CVEs em todos os pontos de aplicação, caminhos automáticos só com aviso,
   código de saída 9.
2. Checagem de CVE.
3. Pin e instalação de npm, yarn e pnpm, a checagem do `packageManager`, regras
   para npm, yarn e pnpm.
4. `nvm init` no modo projeto.
5. `nvm upgrade`.

## 8. Deixado para quem implementa

- Escolha de crates para JSON, geração de schema e faixas semver no estilo npm
  (provavelmente `serde`, `serde_json`, e `schemars` como dev-dependency),
  dentro do `cargo deny`.
- Organização dos módulos, desenho dos ports para a fonte de vulnerabilidades e
  para o registro, e como a política carregada chega aos comandos
  (provavelmente um campo do `Context` definido em `with_real_context`).
- Código de saída para a divergência do `packageManager` e para fixar yarn e
  pnpm juntos, escolhido entre os códigos existentes e documentado no README.
- Como ler as versões do Node do sistema e do npm que vem com o Node.
- Como obter o "agora" para o `minimumReleaseAgeDays` (o port `Clock`
  existente).
- Texto de erros e avisos, além do conteúdo listado na 6.10.
- Como o tarball de cada pacote vira entradas `bin` executáveis.
- Atualizar o `--help` do `nvm init`, criar o do `upgrade`, o README (variáveis,
  comandos, códigos de saída) e o deviations.md.

## 9. Decisões tomadas sem confirmação explícita

- A flag reaproveita o `NVM_MIN_VERSION` para o node; as variáveis novas usam o
  prefixo `NVMRC_`.
- Flags `--min-*` por ferramenta e `--fail-on-severity` existem ao lado de
  `--min-node`; `allow`, `deny`, `ltsOnly` e `minimumReleaseAgeDays` não têm
  flags.
- Um override que afrouxa é um aviso, não um erro.
- A política do repositório se chama `.nvmrc-policy.json` e é encontrada
  subindo os diretórios, como o `.nvmrc`; o `cve.source` dela é ignorado quando
  existe uma política central.
- `NVM_MIN_VERSION` ou `min-version` sozinhos ficam no modo legado (só
  `install`, sem checagem de CVE); qualquer flag ou variável nova liga o modo
  política.
- URLs `http://` de política e de CVE são rejeitadas.
- O formato do índice do Security WG é o contrato para mirrors do `cve.source`;
  CVEs específicos de uma plataforma só contam nela; uma entrada de CVE sem
  severidade conta como `high`; uma severidade desconhecida é uma falha em
  saber.
- A checagem de CVE cobre só o Node.
- Código de saída 9 para falha em saber.
- npm, yarn e pnpm ficam num store compartilhado, não dentro de cada versão do
  Node, e seus pins são versões exatas.
- As regras do npm valem para o npm que vem com o Node quando o npm não está
  fixado.
- No máximo um entre yarn e pnpm por projeto; o sufixo de hash do corepack é
  ignorado na comparação do `packageManager`.
- `init` sem versão escolhe a release permitida mais nova (LTS para o node) e
  atualiza um `.nvmrc` existente no lugar.
- `upgrade` é o nome do comando; ele instala depois de reescrever os arquivos,
  não grava nada a menos que toda ferramenta selecionada tenha alvo, e não
  adiciona ferramentas que o `.nvmrc` não fixa.
