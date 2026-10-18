# nvmrc

[English](README.md) | Português (Brasil)

Um port nativo em Rust do [nvm](https://github.com/nvm-sh/nvm). Três binários
são gerados a partir de uma única biblioteca: `nvmrc`, `nvm` (o mesmo programa,
sob o nome que scripts, CI e ferramentas chamam) e `nvm-exec`.

O nvmrc usa o mesmo layout de `$NVM_DIR` que o nvm, então as versões instaladas
por qualquer uma das ferramentas funcionam com as duas. Ele segue os comandos,
as opções, as mensagens e os códigos de saída do nvm.sh, conferidos contra os
cenários de teste do fork do nvm do autor (`tests/compat/`); os desvios estão
em [docs/deviations.pt-BR.md](docs/deviations.pt-BR.md).

## Instalação

Os releases são gerados a partir de uma tag `v*` por
`.github/workflows/release.yml`, em runners nativos. A versão é a que
`nvm --version` imprime.

Um release publica o release do GitHub, a imagem do GHCR, o crate no
crates.io e a fórmula do Homebrew. O `v0.0.1` foi lançado quando o repositório
era privado, então tem apenas checksums; todo release posterior é assinado e
atestado (veja "Verificando um release").

- Arquivos, `nvmrc-<version>-<target>.tar.gz`, cada um com `nvmrc`, `nvm`,
  `nvm-exec`, `LICENSE` e `README.md`:
  - macOS: `aarch64-apple-darwin`, `x86_64-apple-darwin`.
  - Linux, estático (qualquer distribuição): `x86_64-unknown-linux-musl`,
    `aarch64-unknown-linux-musl`.
  - Linux, glibc 2.34 ou mais nova: `x86_64-unknown-linux-gnu`,
    `aarch64-unknown-linux-gnu`.
- Pacotes, gerados a partir dos binários estáticos (os binários vão para
  `/usr/bin`). A única dependência deles é `ca-certificates`, o repositório de
  certificados confiáveis de que o HTTPS precisa, então instale-os com o
  gerenciador de pacotes, que o traz junto. Eles não são assinados com uma
  chave de distribuição: confira-os como "Verificando um release" mostra.
  - Debian, Ubuntu:
    `sudo apt-get install ./nvmrc_<version>-1_<amd64|arm64>.deb`
  - Fedora, RHEL, Amazon Linux 2023:
    `sudo dnf install ./nvmrc-<version>-1.<x86_64|aarch64>.rpm`
  - Alpine:
    `apk add --update --allow-untrusted nvmrc_<version>-r1_<x86_64|aarch64>.apk`
  - Arch: `sudo pacman -U nvmrc-<version>-1-<x86_64|aarch64>.pkg.tar.zst`
- Docker (linux/amd64, linux/arm64): a imagem contém apenas os binários
  estáticos e os certificados de CA, sem shell e sem libc, então serve para
  comandos avulsos e para copiar os binários para outra imagem:
  `docker run --rm ghcr.io/elioseverojunior/nvmrc:<version> nvmrc --version`,
  ou, em um Dockerfile,
  `COPY --from=ghcr.io/elioseverojunior/nvmrc:<version> /usr/bin/nvm* /usr/local/bin/`.
- Nix: `nix profile install github:elioseverojunior/nvmrc`, ou
  `nix run github:elioseverojunior/nvmrc -- --version`.
- Homebrew: `brew install elioseverojunior/tools/nvmrc` baixa o código-fonte
  com tag do GitHub e o compila com `cargo` (`rust` é uma dependência apenas de
  compilação); `brew install --HEAD elioseverojunior/tools/nvmrc` compila a
  branch `main`. O tap renderiza a fórmula após cada release.
- crates.io: `cargo install --locked nvmrc` instala os três binários em
  `~/.cargo/bin`. O perfil de release abaixo está em `.cargo/config.toml`, que
  uma instalação pelo crates.io não lê, então essa compilação usa o perfil de
  release padrão do Cargo.
- A partir do código-fonte: `cargo install --locked --git
  https://github.com/elioseverojunior/nvmrc` instala os três binários em
  `~/.cargo/bin`, ou
  `cargo build --release --locked` a partir de um clone os coloca em
  `target/release/`. Rust 1.85 ou mais novo (MSRV); o desenvolvimento usa o
  toolchain 1.99 de `rust-toolchain.toml`. O perfil de release (LTO fat,
  `strip = "symbols"`, `panic = "abort"`) está em `.cargo/config.toml`; com o
  rustup, o componente `llvm-tools` que o `rust-toolchain.toml` declara fornece
  o `rust-objcopy` que o strip usa.
- O Windows não é suportado; compilar para ele para com um erro de compilação.
- Uma função `nvm` deixada pelo nvm.sh em um arquivo de inicialização do shell
  esconde o binário `nvm` em shells interativos: `nvmrc doctor` a encontra e
  `nvm migrate` a substitui (veja "Migrando do nvm.sh").
- Para ferramentas que executam `$NVM_DIR/nvm-exec` pelo caminho, crie o link:
  `ln -s "$(command -v nvm-exec)" "$NVM_DIR/nvm-exec"`.

## Verificando um release

O `v0.0.1`, lançado quando o repositório era privado, é verificado apenas com
seus checksums; isso prova que os arquivos correspondem ao `SHA256SUMS` do
release, não quem os gerou
([docs/release.pt-BR.md](docs/release.pt-BR.md) tem a decisão):

```sh
gh release download v0.0.1 --repo elioseverojunior/nvmrc
shasum -a 256 --check --ignore-missing SHA256SUMS # macOS and Linux
```

A partir do primeiro release após o `v0.0.1`, cada arquivo de um release será
atestado pelo GitHub (proveniência de build SLSA), cada binário dentro dos
arquivos será atestado individualmente, e o SBOM
(`nvmrc-<version>.spdx.json`, o grafo de dependências do `Cargo.lock`) será
atestado como tal. Apenas um release com tag é assinado ou atestado; as
verificações abaixo também fixam o workflow de release e a tag, então nada que
uma execução manual ou outro workflow tenha atestado passa. Com o GitHub CLI:

```sh
release=(--repo elioseverojunior/nvmrc
  --signer-workflow elioseverojunior/nvmrc/.github/workflows/release.yml
  --source-ref refs/tags/v0.0.1)
gh attestation verify nvmrc-0.0.1-x86_64-unknown-linux-musl.tar.gz "${release[@]}"
gh attestation verify /usr/bin/nvm "${release[@]}"
gh attestation verify oci://ghcr.io/elioseverojunior/nvmrc:0.0.1 "${release[@]}"
gh attestation verify nvmrc_0.0.1-1_amd64.deb "${release[@]}" \
  --predicate-type https://spdx.dev/Document/v2.3
```

Todo arquivo também é assinado com o Sigstore pelo workflow de release (sem
chave; o bundle é `<file>.sigstore.json`). Com o cosign, verifique o
`SHA256SUMS`, depois os arquivos contra ele, e a imagem:

```sh
identity='^https://github\.com/elioseverojunior/nvmrc/\.github/workflows/release\.yml@refs/tags/v'
issuer=https://token.actions.githubusercontent.com
cosign verify-blob --bundle SHA256SUMS.sigstore.json \
  --certificate-identity-regexp "$identity" --certificate-oidc-issuer "$issuer" SHA256SUMS
shasum -a 256 --check --ignore-missing SHA256SUMS # macOS and Linux
cosign verify ghcr.io/elioseverojunior/nvmrc:0.0.1 \
  --certificate-identity-regexp "$identity" --certificate-oidc-issuer "$issuer"
```

Os comandos usam arrays do bash (bash, zsh). A identidade do cosign nomeia o
workflow de release em uma tag `v*`; o `gh` confere a tag exata. Um arquivo do
macOS baixado com um navegador fica em quarentena pelo Gatekeeper: depois de
verificá-lo, rode `xattr -d com.apple.quarantine nvmrc nvm nvm-exec` nos
binários descompactados.

Nenhuma dessas verificações pode ser executada antes do primeiro release
assinado; os comandos são os que as identidades e os nomes de arquivo do
workflow produzem. Como o release é gerado e no que ele confia:
[docs/release.pt-BR.md](docs/release.pt-BR.md).

## Integração com o shell

Adicione uma linha ao arquivo de inicialização do shell.

bash, em `~/.bashrc`:

```sh
eval "$(nvmrc init bash)"
```

zsh, em `~/.zshrc`:

```sh
eval "$(nvmrc init zsh)"
```

sh, dash e ksh, em `~/.profile` ou no arquivo que `$ENV` nomeia (use `dash` ou
`ksh` no lugar de `sh` para esses shells):

```sh
eval "$(nvmrc init sh)"
```

fish 3.4 ou mais novo, em `~/.config/fish/config.fish`:

```fish
nvmrc init fish | source
```

`--no-use` dispensa a troca para a versão padrão; `--install` a instala quando
ela falta. O snippet define uma função de shell `nvm`. Para `use`,
`deactivate` e `install`, ela abre o descritor 3, o nomeia em
`NVMRC_SCRIPT_FD` e avalia o código de shell que o binário escreve ali, enquanto
as mensagens ficam em stdout e stderr; todo outro comando executa o binário
diretamente. Sem a função, `eval "$(nvm use 18)"` funciona à mão. A linha de
carga chama `nvmrc`, que uma função `nvm` deixada pelo nvm.sh não consegue
interceptar.

## Migrando do nvm.sh

- `nvm doctor [--shell <shell>]` é somente leitura; ele lista todos os lugares
  que ainda carregam o nvm.sh e sai com 1 quando encontra algum.
- `nvm migrate --dry-run` mostra o diff.
- `nvm migrate [--yes]` faz o backup de cada arquivo como
  `<file>.nvmrc-backup-<yyyymmddThhmmssZ>`, comenta as linhas antigas como
  `# [nvmrc-migrated] ...`, adiciona o bloco de init entre
  `# >>> nvmrc init >>>` e `# <<< nvmrc init <<<`, e antes verifica a sintaxe
  com o shell.
- `nvm migrate --undo` restaura o backup mais recente, fazendo antes o backup
  do arquivo atual.

Os lazy loaders são reportados com uma correção sugerida e nunca editados;
`$NVM_DIR` nunca é tocado.

## Variáveis de ambiente

Lidas como o nvm.sh as lê:

- `NVM_DIR`
- `NVM_COLORS`, `NVM_NO_COLORS`, `NVM_HAS_COLORS`, `NO_COLOR`, `TERM`
- `NVM_NODEJS_ORG_MIRROR`, `NVM_IOJS_ORG_MIRROR`, `NVM_AUTH_HEADER`
- `NVM_SYMLINK_CURRENT`, `NVM_INSTALL_LOCK_TIMEOUT`,
  `NVM_INSTALL_LOCK_STALE`, `NVM_INSTALL_THIRD_PARTY_HOOK`,
  `NVM_NO_SOURCE_FALLBACK`, `NVM_MAKE_JOBS`
- `NODE_VERSION` (para o `nvm-exec`), `NODE_PATH`, `MANPATH`, `PREFIX`,
  `npm_config_prefix`/`NPM_CONFIG_PREFIX`, `CC`/`CXX` (builds a partir do
  código-fonte), `HOME`, `PWD`, `PATH`

Para `doctor` e `migrate`: `ENV`, `ZDOTDIR`, `XDG_CONFIG_HOME`,
`NVM_LAZY_LOAD` (uma regra de conflito).

O piso de versão (`NVM_MIN_VERSION`, `$NVM_DIR/min-version`) vem do fork do nvm
do autor, não do nvm-sh/nvm.

Próprias do nvmrc: `NVMRC_SCRIPT_FD`, `NVMRC_SHELL_KIND`, `NVMRC_SHELL` e
`NVMRC_POLICY` (a política de segurança central, veja
[Política de segurança](#política-de-segurança)), `NVMRC_NPM_REGISTRY`,
`NVMRC_NPM_AUTH_HEADER` e `NVMRC_MIN_NPM`/`NVMRC_MIN_YARN`/`NVMRC_MIN_PNPM`
(veja [npm, yarn e pnpm](#npm-yarn-e-pnpm)).

Não suportadas: `NVM_NO_PROGRESS` (não há barra de progresso), `NVM_OFFLINE`
(use `--offline`), configurações de curl e wget (os downloads usam ureq; os
proxies vêm das variáveis `*_PROXY` usuais).

## Política de segurança

Um time de segurança pode decidir quais versões do Node um pipeline pode usar.
A política é um arquivo JSON, verificado contra
[schemas/nvmrc-policy.v1.schema.json](schemas/nvmrc-policy.v1.schema.json); o
design está em
[docs/superpowers/specs/2026-10-17-security-policy-design.pt-BR.md](docs/superpowers/specs/2026-10-17-security-policy-design.pt-BR.md).

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
    }
  }
}
```

As regras do node são `minimum`, `allow` (uma faixa no estilo npm), `deny`,
`ltsOnly` e `minimumReleaseAgeDays`; as configurações de `cve` são descritas em
[Verificação de CVE](#verificação-de-cve). Um campo desconhecido é um erro,
nunca é ignorado. As seções `tools.npm`, `tools.yarn` e `tools.pnpm` aceitam
`minimum`, `allow`, `deny` e `minimumReleaseAgeDays` e são aplicadas; veja
[npm, yarn e pnpm](#npm-yarn-e-pnpm).

Estas fontes participam, nesta ordem:

- `NVMRC_POLICY` ou `--policy <path|https-url>`: a política central. Um caminho
  é lido a partir do diretório de trabalho; `https://` é buscado, e um
  redirecionamento para fora do https é recusado; `http://` é rejeitado. Um
  `NVMRC_POLICY` vazio é tratado como não definido. Quando os dois são dados,
  `--policy` vence e `NVMRC_POLICY` nem chega a ser lido: há uma única política
  central.
- `.nvmrc-policy.json`, encontrado a partir do diretório de trabalho para cima,
  como o `.nvmrc`: a política do repositório.
- `--min-node <version>`: um mínimo próprio.
- `--min-npm`, `--min-yarn`, `--min-pnpm <version>`: o mesmo para essas
  ferramentas, com `NVMRC_MIN_NPM`, `NVMRC_MIN_YARN` e `NVMRC_MIN_PNPM` como
  suas variáveis.
- `NVM_MIN_VERSION`: um mínimo, mesclado com os demais no modo de política.
- `$NVM_DIR/min-version`: o mesmo, vindo do arquivo.

As flags vão antes do subcomando ou em qualquer ponto depois dele até um `--`
(`nvm --policy p use 20`, `nvm use 20 --min-node 20`). Depois de `exec` e `run`
elas só são tomadas imediatamente após o subcomando: qualquer outra opção no
meio as encerra, e uma ocorrência posterior pertence ao programa que elas
executam e é deixada em paz. Uma flag sem valor falha com
`nvm: <flag> needs a value.` e status 127; uma flag dada duas vezes falha com
`nvm: <flag> is given twice.`.

As fontes só se apertam umas às outras, então toda regra assume o valor mais
estrito entre elas: o maior mínimo vence, as faixas permitidas se aplicam todas,
as listas de proibidas se somam, `ltsOnly` fica ligado quando qualquer fonte o
define, e a maior idade de release vence. Um empate nomeia a política central,
depois a política do repositório, e assim por diante pela lista. Uma fonte que
pede menos é ignorada com um aviso.

Dois modos:

- Legado: apenas `NVM_MIN_VERSION` ou `$NVM_DIR/min-version`. É exatamente o
  comportamento antigo, verificado somente no `install`.
- Política: qualquer política central, política do repositório ou flag de
  política (`--min-node` incluída). `NVM_MIN_VERSION` e
  `$NVM_DIR/min-version` passam então a contar também como camadas, e podem ser
  a fonte que uma violação nomeia. As regras são aplicadas em `install`, `use`
  (`use system` incluído), `exec` e `run`. A inicialização do shell (`__auto`)
  apenas avisa e segue em frente; suas buscas de política e de índice têm uma
  única tentativa de no máximo 5 segundos, para que uma rede silenciosa nunca
  prenda o shell.

Uma violação para com o status 7:

```text
nvm: node v18.19.0 (from /w/app/.nvmrc) is not allowed by policy:
  - below the minimum v20.11.1 (from --min-node)
Raise /w/app/.nvmrc to an allowed version.
```

A versão vem do caminho do `.nvmrc`, de `the command line`, de `system` ou de
`the active version`. Uma política que não pode ser verificada nunca é deixada
passar: uma política ilegível ou inválida, um índice inacessível para uma regra
de data de release ou de LTS, `--offline` com tal regra, uma versão do node do
sistema ou um arquivo `min-version` ilegível, tudo isso para com o status 9.

### Verificação de CVE

No modo de política, `install`, `use`, `exec` e `run` também consultam a versão
do node nos advisories de segurança do Node.js. A política pode definir:

```json
{
  "schemaVersion": 1,
  "cve": { "failOnSeverity": "high", "source": "vuln/index.json" }
}
```

Uma versão é afetada por um advisory quando está em `vulnerable` e não está em
`patched`. Um advisory que lista `affectedEnvironments` só conta quando nomeia
`all` ou este SO; sem plataforma conhecida, ou com uma lista vazia, ele conta.
Uma severidade ausente ou `unknown` é `high`; qualquer outro valor que não seja
`critical`, `high`, `medium` ou `low` significa que o índice não pode ser
compreendido. Um advisory sem ids de CVE é nomeado `advisory <id>`. As versões
do io.js não são verificadas.

O limite é a menor severidade que falha. Ele vem de
`--fail-on-severity <critical|high|medium|low|none>`,
`NVMRC_FAIL_ON_SEVERITY` e `cve.failOnSeverity` na política central e na
política do repositório. O mais estrito vence (`low` é mais estrito que `high`,
`none` é o mais frouxo), o padrão é `high`, e uma sobrescrita mais frouxa é
ignorada com um aviso. Um `NVMRC_FAIL_ON_SEVERITY` não vazio liga o modo de
política, como faz uma flag.

```text
nvm: node v22.20.0 (from the command line) is not allowed by policy:
  - CVE-2026-58043 (high); the limit is high (from NVMRC_FAIL_ON_SEVERITY)
Use an allowed version.
```

Uma ocorrência igual ou acima do limite para com o status 7. As ocorrências
abaixo dele apenas imprimem `nvm: warning: node vX has known vulnerabilities
below the limit <limit>: ...` e o status não muda; com `none`, toda ocorrência
é reportada dessa forma.

O índice é
`https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json`,
a menos que `cve.source` (uma URL ou um caminho) diga outra coisa. Quando existe
uma política central, só o `cve.source` dela conta, mesmo que a política do
repositório nomeie um; sem política central, usa-se o da política do
repositório. Um `cve.source` relativo é lido ao lado do arquivo de política que
o contém, e é inválido em uma política buscada por https. `use --save` no modo
de política grava o `.nvmrc` somente depois que a política permite a versão.

Um índice que não pode ser lido ou compreendido para com o status 9
(`nvm: cannot check the policy: the vulnerability index <location> is not
valid: ...` quando é inválido). Se as regras do node já falham, o status
permanece 7 e um `nvm: warning: cannot check known vulnerabilities: ...` é
acrescentado. `--offline` com um índice https para com o status 9 antes de
qualquer requisição; um índice local funciona offline. A inicialização do shell
(`__auto`) avisa e segue em frente. O modo legado nunca busca o índice.

## npm, yarn e pnpm

Um `.nvmrc` pode fixar os gerenciadores de pacotes ao lado da versão do node:

```text
20.11.1
npm=10.9.0
pnpm=9.12.0
```

Cada pin é um `major.minor.patch` exato, só com dígitos: sem `v`, sem
prerelease, sem faixa. Yarn e pnpm juntos é um erro, e uma ferramenta fixada
duas vezes também. Os pins vêm do `.nvmrc` mais próximo e são lidos por
`install`, `use`, `exec` e `run`, com ou sem política, mesmo quando a versão do
node é dada na linha de comando.

Quando yarn ou pnpm está fixado e há um `package.json` ao lado desse `.nvmrc`,
o seu `packageManager` deve ser `<name>@<version>` para a mesma ferramenta e a
mesma versão (um sufixo `+<hash>` é ignorado). O nvmrc nunca grava o
`package.json`. Um pin inválido, um `packageManager` divergente e uma
ferramenta fixada que não está instalada (em um `use`, `exec` ou `run`
explícito) param com o status 3; os caminhos automáticos (`__auto`, o snippet
de init) apenas avisam.

O `install` instala as ferramentas fixadas depois do node e antes de ativá-lo.
Elas vão para `$NVM_DIR/package-managers/<tool>/<version>/`, com `package/` (o
pacote descompactado) e `bin/` (os comandos), compartilhados por todas as
versões do node. Os pacotes são `npm`, `pnpm`, `yarn` abaixo de 2.0.0 e
`@yarnpkg/cli-dist` a partir de 2.0.0. O tarball deve corresponder a uma
entrada `sha512` de `dist.integrity`, um comando não pode apontar para fora do
seu pacote, e uma instalação que falha mantém a anterior. Com `--offline`, uma
ferramenta fixada que não está no store falha. Uma ferramenta fixada que falha
ao instalar, ou que falta sob `--offline`, faz o `install` sair com 1; os
caminhos automáticos apenas avisam.

`use`, `exec` e `run` em um node instalado colocam o `bin` de cada ferramenta
fixada à frente do node no `PATH`, na ordem npm, yarn, pnpm e depois node, e
descartam as entradas de ferramentas de outros projetos. `use system` não
adiciona nenhuma, e `deactivate` as remove.

`install --save` e `use --save` (`-w`) substituem apenas a linha de versão do
`.nvmrc`, mantendo seu comentário final, e preservam as linhas de pin e todas
as outras linhas.

O registro é `NVMRC_NPM_REGISTRY` (padrão `https://registry.npmjs.org`, somente
https). Coloque as credenciais do registro em `NVMRC_NPM_AUTH_HEADER`, não na
URL: as informações de usuário de uma URL podem aparecer em mensagens de erro.
`NVMRC_NPM_AUTH_HEADER` é enviado apenas ao host do registro.
`NVM_AUTH_HEADER` nunca é enviado ao registro do npm, a um host de tarball nem
ao índice de vulnerabilidades embutido; ele continua indo para os mirrors do
Node, para uma política central e para um `cve.source` configurado.

As regras de política para essas ferramentas (`minimum`, `allow`, `deny`,
`minimumReleaseAgeDays`) vêm das mesmas fontes que as do node, com
`--min-npm`, `--min-yarn` e `--min-pnpm`, e se apertam da mesma forma;
`ltsOnly` não se aplica e os CVEs delas não são verificados. Uma ferramenta é
verificada quando fixada. Sem um pin de npm, o npm embutido no node em uso é
verificado, e apenas quando existe uma regra de npm. Com o node do sistema
nenhum pin está em vigor: o npm ao lado do node do sistema é verificado, e
yarn e pnpm não. `install --offline` com uma regra de npm e sem pin de npm
precisa que esse node já esteja instalado, já que de outro modo o npm embutido
vem do índice remoto (saída 9). A idade do release usa os metadados `time` do
registro. Uma violação sai com 7, uma verificação que não pode ser feita sai
com 9.

## Iniciando um projeto

`nvm init` sem nome de shell e com uma ou mais das opções `--node[=<version>]`,
`--npm[=<version>]`, `--yarn[=<version>]` e `--pnpm[=<version>]` fixa as
ferramentas no diretório de trabalho:

```sh
nvm init --node --pnpm
nvm init --node=22.11.0 --npm=10.9.0
nvm --min-pnpm 9.12.0 init --pnpm
```

Sem uma versão, uma ferramenta recebe o release permitido mais novo. Para o
node, esse é o release LTS permitido mais novo, com ou sem `ltsOnly` (por
versão, não por data de release). Para npm, yarn e pnpm, é o
`major.minor.patch` estável permitido mais novo no registro; prereleases nunca
são escolhidas, e o yarn vem de `@yarnpkg/cli-dist`. Com uma versão, ela deve
ser exata (node: `20.11.1` ou `v20.11.1`; as demais: só dígitos, como os pins),
publicada e permitida (yarn abaixo de 2.0.0 é o pacote `yarn`). Sem política,
toda versão publicada é permitida; o piso legado se aplica apenas ao
`install`. As flags de política (`--policy`, `--min-*`) se aplicam como no
`install`.

Quando a política tem uma regra de npm e o npm não será fixado (sem linha
`npm=` no `.nvmrc` e sem `--npm` na mesma execução), o node mais novo é
escolhido apenas entre os releases cujo npm embutido (a coluna `npm` do índice
de releases) a política permite, já que esse npm é o que está em vigor; um
release cujo npm embutido é desconhecido é pulado. Um node exato dado com
`--node=<version>` é recusado pelas mesmas razões (saída 7), e o bloco as nomeia
depois das regras do próprio node:

```text
nvm: node v20.18.0 (from the command line) is not allowed by policy:
  - the npm it bundles, npm 10.4.0, is below the minimum 10.5.0 (from policy /policy.json)
Use an allowed version.
```

ou `- the npm it bundles is unknown, and npm has rules`. Quando o npm não pode
ser verificado (digamos, a data do registro de que uma regra de idade precisa
está indisponível) em um node que suas próprias regras já recusam, a recusa
permanece (saída 7) e `nvm: warning: cannot check npm: <reason>` segue o
bloco, como no gate do `install`; sem nenhum outro motivo, ele falha com saída
9.

`nvm init` grava o `.nvmrc` no diretório de trabalho, criando-o quando ausente,
e altera apenas as linhas pedidas: a linha de versão simples para o node e a
linha `npm=`, `yarn=` ou `pnpm=` para as demais. Toda outra linha, comentário e
a ordem são mantidos, assim como um comentário na linha de versão. Quando yarn
ou pnpm é pedido e existe um `package.json` no diretório de trabalho, seu
`packageManager` é definido como `<name>@<version>`; nada mais nele muda, um
valor que já corresponde (um `+<hash>` do corepack incluído) é deixado como
está, e um `package.json` ausente não é criado. Toda verificação roda antes de
qualquer gravação, então uma recusa deixa os dois arquivos intocados:

```text
Wrote /p/.nvmrc: node v22.11.0, pnpm 9.12.0
Set packageManager to "pnpm@9.12.0" in /p/package.json (was "pnpm@9.1.0")
```

A linha `Set packageManager` aparece apenas quando o valor muda, e nomeia o valor
que substituiu, se houver. Quando a versão do node escolhida tem
vulnerabilidades conhecidas abaixo do limite de severidade, é impresso o mesmo
aviso do `install`.

Pedir yarn quando o `.nvmrc` já fixa pnpm (ou o inverso) é recusado, nunca
trocado, e o mesmo vale para uma linha de pin malformada até que seja
corrigida; ambos são recusados antes de qualquer requisição.

Um arquivo que não pode ser lido ou gravado sai com 1. O `package.json` é
gravado primeiro; quando a gravação do `.nvmrc` falha em seguida, o
`package.json` recebe de volta seu texto original (se isso também falhar, uma
linha diz que ele já havia sido atualizado).

Códigos de saída: 1 para um arquivo que não pode ser lido ou gravado; 3 para
uma versão que não é exata ou não foi publicada, um conflito de pins, ou um pin
ou `package.json` malformado; 6 para um erro de uso (um nome de shell com uma
flag de projeto, `--yarn` com `--pnpm`, uma flag repetida, `--no-use` ou
`--install` com uma flag de projeto); 7 quando a versão não é permitida ou
nenhum release é permitido; 9 quando o índice, o registro ou a política não
podem ser lidos.

## Atualizando um projeto

`nvm upgrade` leva as ferramentas fixadas no `.nvmrc` mais próximo (procurado
para cima, como o `use` faz) a releases permitidos mais novos, e reescreve esse
arquivo e o `package.json` ao lado dele:

```sh
nvm upgrade
nvm upgrade --pnpm
nvm upgrade --node --major
nvm upgrade --pnpm --major=10
```

Sem flag de ferramenta, toda ferramenta que o arquivo fixa é selecionada: o
node quando há uma linha de versão simples, e npm, yarn e pnpm quando há uma
linha `<tool>=`. `--node`, `--npm`, `--yarn` e `--pnpm` selecionam apenas
essas; uma flag para uma ferramenta que o arquivo não fixa é recusada (saída
6), e uma flag repetida também.

Por padrão, cada ferramenta vai para o release permitido mais novo na major que
já usa, e nunca cruza uma major por conta própria. `--major` permite o release
permitido mais novo em qualquer major. `--major=<N>` escolhe o release
permitido mais novo na major `N`; exige exatamente uma ferramenta selecionada,
o que também significa uma ferramenta fixada quando nenhuma flag de ferramenta
é dada (caso contrário, saída 6). Para o node, qualquer release permitido pode
ser escolhido, inclusive um não LTS, a menos que a política defina `ltsOnly`.
npm, yarn e pnpm usam apenas releases estáveis `major.minor.patch`; o yarn lê o
pacote `yarn` para a major 1 e `@yarnpkg/cli-dist` para a major 2 e posteriores
(`--major` pesquisa ambos). As flags de política (`--policy`, `--min-*`) se
aplicam como no `install`. Uma versão atual que a política não permite vai para
o alvo mesmo quando o alvo é mais baixo. Como no `nvm init`, com uma regra de
npm e sem linha `npm=`, um release do node só é escolhido quando seu npm
embutido é permitido (e conhecido).

Uma linha de node que é uma versão parcial (`20`, `v20.11`) usa essa major e é
substituída pelo alvo exato. Uma linha de node que não é uma versão (`lts/*`,
`node`, uma versão `iojs-`) é recusada quando o node é selecionado, saída 3:

```text
nvm upgrade: /p/.nvmrc names node as "lts/*", not a version; use nvm init --node to pin one.
```

Com `--pnpm` (ou outra flag de ferramenta sem `--node`), a linha de alias é
deixada em paz e a outra ferramenta é atualizada. Sem um `.nvmrc`, `nvm upgrade`
diz `nvm upgrade: no .nvmrc found; run nvm init first.` (saída 3). Um `.nvmrc`
sem linha de node nem pin é recusado antes de qualquer requisição, saída 3:

```text
nvm upgrade: /p/.nvmrc pins nothing; use nvm init to add a tool.
```

Quando uma ferramenta selecionada não tem release permitido na sua major,
nada é gravado e o comando sai com 7. Para cada uma dessas ferramentas, ele
imprime `nvm upgrade: no allowed pnpm release in major 9.`, o bloco de política
do release mais novo daquela major com as regras que ele quebra e de onde elas
vêm, e uma dica:

```text
nvm: pnpm 9.12.0 (the newest in major 9) is not allowed by policy:
  - below the minimum 10.0.0 (from policy /policy.json)
The lowest major with an allowed release is 10. Run `nvm upgrade --pnpm --major=10`, or `nvm upgrade --pnpm --major` for the newest.
```

ou `No major has an allowed pnpm release.` Com `--major`, a primeira linha é
`nvm upgrade: no allowed pnpm release.` e o bloco nomeia
`(the newest release)`. Para o node, os motivos do npm embutido seguem as
regras do próprio node, como no `nvm init` (veja acima). Para o yarn, a menor
major considera os dois pacotes, então a partir do yarn 1 a dica pode apontar
para `--major=4`, e a partir do yarn 4 para `--major=1`.

Os arquivos são gravados primeiro, depois as mudanças são impressas, na ordem
node, npm, yarn, pnpm; uma ferramenta que já está no alvo é reportada como
atualizada:

```text
node v20.11.1 -> v20.18.0
pnpm 9.12.0 is up to date
```

Se nada mudou, a saída é 0 e nada é gravado nem instalado. Em uma falha de
gravação, aparece apenas o erro. A gravação segue o `nvm init`: os dois textos
são calculados primeiro, o `package.json` é gravado primeiro e recebe de volta
seu texto original quando a gravação do `.nvmrc` falha.

Depois de gravar, o `install` roda como o `nvm install` sem argumento rodaria:
lê o `.nvmrc` reescrito e instala o node e os pins. Um `.nvmrc` sem linha de
node (como o `nvm init --pnpm` grava) tem instalados apenas os pins que esta
execução moveu; a política os permitiu, e os pins deixados em paz não são
instalados. Se isso falha, por seu status ou por um erro como uma política que
não pode ser verificada, as linhas de mudança e o erro são impressos, seguidos
de:

```text
nvm upgrade: /p/.nvmrc is updated, but the install failed; run nvm install to finish.
```

e o comando sai com o status do install ou o código do erro.

Códigos de saída: 1 para um arquivo que não pode ser lido ou gravado; 3 para
ausência de `.nvmrc`, um que não fixa nada, uma linha de alias de node, ou uma
linha de pin inválida; 6 para um erro de uso; 7 quando não existe release
permitido na major; 9 quando o índice, o registro ou a política não podem ser
lidos; o status do install quando o install falha.

## Comandos

Como no nvm.sh: `version`, `current`, `ls`/`list`, `ls-remote`/`list-remote`,
`cache dir|clear`, `install`/`i` (binários, e `-s` a partir do código-fonte),
`install-latest-npm`, `reinstall-packages`/`copy-packages`, `uninstall`,
`version-remote`, `which`, `alias`, `unalias`, `use`, `deactivate`,
`set-colors`, `exec`, `run`.

Novos: `init`, `upgrade`, `doctor`, `migrate`.

Não disponíveis: `unload`, `debug`, o texto de `--help` e o número de
`--version` do nvm.sh, `self-install`/`self-update` (v2), plataformas legadas
(v2), Windows.

## Desvios

O comportamento segue o nvm.sh sempre que um teste pode compará-lo. As
diferenças conhecidas são listadas, com o motivo, em
[docs/deviations.pt-BR.md](docs/deviations.pt-BR.md). As mais visíveis são o
texto de ajuda, `--version`, e a ordem das mensagens quando stdout e stderr são
lidos juntos.

## Códigos de saída

- 0: sucesso.
- 1: falha.
- 2: alvo ausente (download do binário falhou sem fallback para o
  código-fonte, `alias lts/<missing>`, `reinstall-packages` a partir da versão
  em uso).
- 3: versão inválida ou desconhecida; um pin `npm=`/`yarn=`/`pnpm=` inválido,
  um `packageManager` que discorda do pin, ou uma ferramenta fixada que não
  está instalada em um `use`, `exec` ou `run` explícito.
- 4: `--reinstall-packages-from` a versão que está sendo instalada.
- 5: `--reinstall-packages-from` uma versão não instalada.
- 6: opções que não podem ser combinadas.
- 7: abaixo do piso de versão, ou não permitida pela política; ou um piso
  inválido.
- 8: loop de alias.
- 9: a política não pôde ser verificada (veja
  [Política de segurança](#política-de-segurança)).
- 11: uma configuração de prefixo do npm com a qual o nvm não consegue
  trabalhar.
- 33: o hook de instalação de terceiros declarou sucesso.
- 55: opção não suportada.
- 127: erro de uso, comando desconhecido, comando não encontrado, ou nenhuma
  versão do sistema.

`exec`, `run`, `nvm-exec` e o hook de instalação repassam o status do seu
programa (128+n após o sinal n). 10 e 42 nunca são produzidos (internos ao
nvm.sh, e `nvm debug`).

## Desenvolvimento

`mise run setup` instala as ferramentas (e pode usar o Homebrew). Depois:

- `mise run fmt`, `lint` (clippy, rumdl, actionlint, hadolint), `test`,
  `test:msrv`, `deny`, `audit`.
- `mise run check` executa todos os gates, como o CI faz.
- `mise run docker:build` e `docker:test` executam a suíte em uma imagem Debian
  com bash, zsh, dash, ksh e fish.
- `mise run compat:capture --nvm-sh <path>` reexecuta o contrato por um nvm.sh
  de verdade e imprime o que difere.

Releases: defina `version` em `Cargo.toml`, rode `cargo check` (ele atualiza o
`Cargo.lock`), faça o commit dos dois (`chore(release): v0.2.0`), depois
`git tag -s v0.2.0 -m "nvmrc 0.2.0"` e envie a tag. O workflow recusa uma tag
que não seja `v` mais a versão do `Cargo.toml` (`mise run release:check`). Uma
execução manual do workflow de release é, por padrão, uma execução de teste
(dry run), e `mise run release:snapshot` executa localmente o mesmo build,
empacotamento e smoke tests em `dist/` (requer Docker).

Os commits seguem Conventional Commits e são assinados com GPG. O CI está em
`.github/workflows/ci.yml`.

## Licença

MIT, veja `LICENSE`; o aviso de copyright do nvm é mantido.
