# Strahler para RustGIS

Plugin WebAssembly para o RustGIS que calcula a **ordem de Strahler** dos segmentos de uma rede linear, a partir da topologia (sem MDE). É um port do plugin QGIS [LucasDaniel28/Strahler3.0](https://github.com/LucasDaniel28/Strahler3.0), que por sua vez adapta o trabalho original de Alexandre Delahaye.

## Ferramenta

**Analyze › Tools › Strahler › Strahler Order** (`plugin:strahler/strahler-order`)

| Parâmetro | Descrição |
|---|---|
| `input` | Camada de linhas (`LineString`). |
| `outlet` | Coordenadas `x,y` da extremidade livre do segmento raiz (na CRS da camada). |
| `field` | Nome do campo de saída. Padrão: `strahler`. |

O resultado é uma **nova camada** (`<camada> strahler`), com os mesmos segmentos e atributos, mais o campo com a ordem de cada segmento.

## Requisitos dos dados

- Segmentos divididos nas junções, com as extremidades coincidentes.
- Geometrias `LineString`. Multipart são ignoradas.
- O `outlet` precisa ser a extremidade livre de exatamente um segmento, e a outra extremidade desse segmento deve estar ligada à rede.
- Limpe a seleção da camada antes de executar: a ferramenta recebe só os segmentos selecionados.

## Regras

- Cabeceiras recebem ordem 1.
- Quando dois segmentos de mesma ordem se encontram, o segmento a jusante recebe a ordem seguinte.
- Quando ordens diferentes se encontram, mantém-se a maior.

## Compilar e empacotar

```bash
rustup target add wasm32-unknown-unknown
cargo test                                   # testes do algoritmo (ver nota abaixo)
cargo build --release --target wasm32-unknown-unknown
```

Depois copie `target/wasm32-unknown-unknown/release/strahler.wasm` para `plugin.wasm` ao lado de `plugin.json` e compacte os dois em `strahler-0.2.0.rgplugin`.

Nota: no Windows, `cargo test` do crate pode falhar no link porque o SDK declara os imports do host. A lógica do algoritmo (`src/strahler.rs`) não depende do SDK e pode ser testada sozinha com `rustc --edition 2024 --test src/strahler.rs`.

## Licença

GPL-2.0-or-later, como o plugin original. Créditos a Alexandre Delahaye e a LucasDaniel28.
