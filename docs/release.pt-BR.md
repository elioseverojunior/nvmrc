# Fazendo o release do nvmrc

[English](release.md) | Português (Brasil)

Como um release é construído e no que ele confia. Os comandos para instalar e
para verificar um release estão no [README](../README.pt-BR.md).

## Visão geral

- Uma tag `v*` igual a `v` mais a versão do `Cargo.toml` dispara
  `.github/workflows/release.yml`; `mise run release:check` garante isso.
- O commit da tag precisa estar na branch padrão: `mise run
  release:check:ci` se recusa a publicar uma tag cujo commit
  `git merge-base --is-ancestor` não encontra lá. Um dry run manual pode
  construir qualquer branch.
- Uma execução manual é um dry run por padrão: ela compila, empacota e roda os
  smoke tests, e para aí. Somente uma execução que publica assina e atesta,
  de modo que toda assinatura e toda atestação nomeiam um release com tag; um
  repositório privado publicaria sem nenhuma das duas (veja "Um repositório
  privado").
- A publicação é ordenada para que uma falha nunca deixe um release publicado
  pela metade: o release do GitHub é criado como rascunho
  (`mise run release:github:draft`), depois a imagem é enviada, executada por
  digest, assinada e atestada (somente em um repositório público), e o último
  passo publica o rascunho. Uma nova execução do job atualiza o rascunho
  (arquivos substituídos com `--clobber`). Só então o tap é solicitado a gerar
  a fórmula e o crate é publicado, cada um a partir de um release que já é
  público.
- Os pacotes dependem de `ca-certificates`; os smoke tests os instalam com o
  gerenciador de pacotes de cada distribuição e executam `nvm ls-remote` via
  HTTPS no contêiner.
- Todo passo é uma tarefa `mise run release:*`, de modo que
  `mise run release:snapshot` executa localmente a mesma compilação,
  empacotamento e smoke tests em `dist/`.
- Não há GoReleaser: os pacotes vêm do nfpm (`packaging/nfpm.yaml`), e o
  restante das tarefas do mise. O GoReleaser compila tudo em um único host,
  enquanto o release compila em runners nativos e atesta os bytes exatos que
  distribui.
- O tap do Homebrew (`elioseverojunior/homebrew-tools`) é dono da fórmula, que
  compila o nvmrc a partir do tarball do código-fonte com tag usando `cargo`.
  `mise run release:homebrew` executa o `bump.yml` do tap, que a renderiza e
  abre um pull request lá. O passo só executa quando a variável do repositório
  `NVMRC_HOMEBREW_TAP` está definida, com o secret `HOMEBREW_TAP_TOKEN`
  autorizado a executar workflows (`actions: write`) no tap.
- O crate vai para o crates.io com Trusted Publishing: o job `crate` obtém um
  token de curta duração de `rust-lang/crates-io-auth-action` e executa
  `mise run release:crate`; nenhum token é armazenado. O crates.io só confia em
  um workflow para um crate que já existe, então o proprietário publica a
  primeira versão manualmente (`cargo publish --locked`), adiciona
  `release.yml` (com o ambiente `crates-io`) como publicador confiável do crate
  e define a variável do repositório `NVMRC_CRATES_IO` como `true`; até então o
  job é ignorado. `mise run release:crate --dry-run`, que o `release:snapshot`
  executa, empacota e compila o crate sem enviá-lo.

## Um repositório privado

O repositório é público. Ele era privado quando o `v0.0.1` foi lançado, e o
workflow ainda trata um repositório privado da mesma forma: uma tag publica,
mas somente quem pode ler o repositório consegue baixar os arquivos.

Um release privado tem apenas checksums, sem assinaturas e sem atestações de
proveniência. `mise run release:check:ci` imprime `provenance=false` quando
`github.event.repository.private` é true; o job `sign-and-attest` e os passos
`cosign sign` e `actions/attest` da imagem são então ignorados, o release é
publicado a partir dos arquivos não assinados, e `mise run release:notes
--provenance false` informa isso nas notas. A decisão:

- `actions/attest` falharia: as atestações de artefato do GitHub em um
  repositório privado exigem o GitHub Enterprise Cloud (o README da action).
- O cosign 3.1.3 (`mise.lock`) consegue assinar sem o log do Rekor: uma
  configuração de assinatura criada com `cosign signing-config create
  --with-default-services --no-default-rekor` não tem log de transparência
  (`--tlog-upload=false` está obsoleto em favor dela), o bundle carrega um
  timestamp no lugar, e `cosign verify-blob --insecure-ignore-tlog` o aceita.
  Mas a assinatura keyless ainda pede um certificado ao Fulcio, e o Fulcio
  acrescenta cada certificado que emite ao seu log público de transparência de
  certificados; para um workflow, esse certificado nomeia o repositório, o
  `release.yml` e a tag. A assinatura keyless publicaria, portanto, o nome do
  repositório de qualquer forma.
- Um par de chaves mantido em um secret evitaria ambos os logs, mas é uma
  chave que o proprietário precisa guardar e rotacionar, e ela comprova menos
  que a identidade do workflow. Não está configurado.

Assim, um release privado é verificado somente com seus checksums, baixados
por HTTPS autenticado:

```sh
gh release download v0.0.1 --repo elioseverojunior/nvmrc
shasum -a 256 --check --ignore-missing SHA256SUMS # macOS and Linux
```

Isso mostra que os arquivos correspondem ao `SHA256SUMS` do release, não quem
os construiu: qualquer pessoa com permissão de escrita no release pode
substituir ambos. As atestações de proveniência e as assinaturas ficam
indisponíveis enquanto o repositório for privado. Nada no workflow mudou quando
ele se tornou público: a primeira tag após a mudança é assinada e atestada; o
`v0.0.1` continua sem assinatura.

## Protegendo as tags

A verificação de ancestralidade impede uma tag em um commit não mesclado, mas
não impede quem pode fazer push de tags de movê-las ou excluí-las. O
proprietário deve adicionar um ruleset de tags para `v*` nas configurações do
repositório (Rules, Rulesets) que restrinja a criação aos mantenedores e
bloqueie atualizações e exclusões. Isso não é configurado a partir deste
repositório.

## Cadeia de suprimentos

Toda action de terceiros nos workflows, incluindo a
`elioseverojunior/rust-toolchain` do proprietário, é fixada em um SHA de commit
completo, com o release a partir do qual foi resolvido como comentário final
(`uses: actions/checkout@<sha> # v7.0.1`). Mover uma tag no upstream, portanto,
não muda nada que execute nos jobs que produzem os binários assinados.
`mise run actions:pins` relata pins desatualizados (o workflow
`action-pin-checker` o executa semanalmente) e `mise run actions:pins --
--write` os move para o release mais recente dentro da mesma versão major; uma
nova versão major é apenas relatada, e é aplicada com `--apply-major` após
revisão. O Dependabot também propõe as atualizações semanalmente.

Os jobs de release não restauram nenhum cache: a action da toolchain Rust roda
sem um, e todo passo `jdx/mise-action` define `cache: false`, de modo que
cosign, nfpm, syft e git-cliff são baixados e conferidos com o `mise.lock` a
cada execução. Nada que uma execução anterior tenha deixado pode alcançar um
arquivo assinado. Os checkouts definem `persist-credentials: false`.

Os certificados CA da imagem vêm de `alpine:3.24.2`, fixado por tag de versão
no `Dockerfile`: um Alpine mais novo é uma atualização revisada, não uma
mudança silenciosa. O digest não é fixado, então uma tag reconstruída é aceita
como está; em vez disso, o job de release executa a imagem que enviou, por
digest, antes de assinar e atestar esse digest (em um repositório público). Os
builds não são reproduzíveis bit a bit (os timestamps de arquivos e pacotes
diferem entre execuções); as atestações e os checksums nomeiam os bytes exatos
que foram publicados.
