# Strahler for RustGIS

A WebAssembly plugin for RustGIS that computes the **Strahler stream order** of the segments of a line network, from its topology alone (no DEM).

It is an adaptation for RustGIS of the QGIS plugin [Strahler3.0](https://github.com/LucasDaniel28/Strahler3.0) by LucasDaniel28, which in turn adapts the original work of Alexandre Delahaye. The RustGIS port is by Ariel S. Rossi.

## Tools and commands

| Item | Id | Where | What it does |
|---|---|---|---|
| Strahler Order | `plugin:strahler/strahler-order` | Tools › Strahler | Returns a new line layer whose segments carry their order in the `Order_Strahler` field. |
| Help | `plugin:strahler/strahler-help` | Tools › Strahler | Shows a table with the steps for using Strahler Order. |
| Color Strahler Orders | `plugin:strahler/style-strahler` | Plugins ribbon | Colors each order class and widens the lines of higher orders. |

### Strahler Order

| Parameter | Description |
|---|---|
| `input` | A line layer (`LineString`, or a single-part `MultiLineString`). |
| `outlet` | Latitude and longitude of the root segment's free end, written as `lat,lon` (WGS 84). |
| `tolerance` | Snap tolerance in degrees (default `0.0001`, about 11 m). |

The result is a new layer named `<input> strahler`, with the same segments and attributes plus the `Order_Strahler` field.

### Color Strahler Orders

The ribbon button sends no parameters. The command colors the **active layer** (the one selected in the Layers pane), which must have an `Order_Strahler` field. With no active layer, it uses the last layer that has that field.

- Each order gets a class with the default colors `#1f77b4`, `#ff7f0e`, `#2ca02c` and `#d62728`, repeated when there are more orders.
- Line widths: order 1 is 2 pt, order 2 is 3 pt, order 3 is 4 pt and order 4 is 5 pt.

## Data requirements

- The network must be split at its junctions, with segments meeting at their ends.
- Multipart lines are skipped.
- The outlet must be the free end of exactly one segment, and that segment's other end must connect to the network.
- Clear the layer's selection before running the tool: the tool then receives only the selected segments.

## Rules

- Headwaters get order 1.
- Where two segments of the same order meet, the downstream segment gets the next order.
- Where segments of different orders meet, the larger order is kept.

## Build and package

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

Copy `target/wasm32-unknown-unknown/release/strahler.wasm` to `plugin.wasm` next to `plugin.json`, then zip the two files into `strahler-0.2.4.rgplugin`.

The algorithm (`src/strahler.rs`) and the color helpers (`src/style.rs`) do not depend on the RustGIS SDK. Test them on their own with:

```bash
rustc --edition 2024 --test src/strahler.rs
rustc --edition 2024 --test src/style.rs
```

On Windows, `cargo test` for the whole crate can fail at link time, because the SDK declares host imports.

## License

GPL-2.0-or-later, as the original plugin. Credits: Alexandre Delahaye (original QGIS plugin), LucasDaniel28 (Strahler3.0), and Ariel S. Rossi (RustGIS port).
