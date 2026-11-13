//! Strahler: the Strahler order of each segment of a line network.
//!
//! Port of the QGIS plugin github.com/LucasDaniel28/Strahler3.0 (originally by Alexandre
//! Delahaye). The **Strahler Order** tool reads a line layer and returns a new layer whose
//! segments carry their order in a field. The outlet is the free end of the root segment, given
//! as `lat,lon` in degrees (the RustGIS standard; features are in WGS 84); the walk goes upstream
//! from there.
//!
//! The tool gets only the selected features when a layer has a selection, so clear the selection
//! before running it.

mod strahler;
mod style;

/// The attribute that holds each segment's Strahler order, in the output and the styled layer.
const FIELD: &str = "Order_Strahler";

use rustgis_plugin::{Kind, Request, ToolOutput, Value, geojson, host, json};

fn handle(req: Request) -> Result<Value, String> {
    match (req.kind, req.id.as_str()) {
        (Kind::Tool, "strahler-order") => strahler_order(&req),
        (Kind::Tool, "strahler-help") => strahler_help(),
        (Kind::Command, "style-strahler") => style_strahler(&req),
        (_, other) => Err(format!("Strahler has no '{other}'")),
    }
}

rustgis_plugin::export!(handle);

/// Colors a layer's Strahler classes: one graduated class per order, each in its color.
/// The layer is the one named by `layer`, or else the active layer with a `strahler` field (the
/// last such layer when none is active). The ribbon button sends no parameters.
fn style_strahler(req: &Request) -> Result<Value, String> {
    let layer = match req.str("layer").filter(|l| !l.is_empty()) {
        Some(name) => name.to_string(),
        None => strahler_layer()?,
    };
    let colors = style::parse_colors(req.str("color").unwrap_or(""))?;

    let collection = host::features(&layer, false)?;
    let features = collection["features"].as_array().or(collection.as_array()).ok_or("RustGIS sent no features")?;
    let orders = style::distinct_orders(features.iter().filter_map(|f| f["properties"][FIELD].as_u64()));
    if orders.is_empty() {
        return Err(format!("'{layer}' has no {FIELD} values. Run Strahler Order first."));
    }
    if orders.len() > style::MAX_CLASSES {
        return Err(format!("'{layer}' has {} orders; at most {} classes are supported.", orders.len(), style::MAX_CLASSES));
    }

    host::run(
        "layer.symbology",
        json!({
            "layer": layer,
            "type": "graduated",
            "field": FIELD,
            "method": "manual",
            "classes": orders.len(),
            "breaks": style::breaks(&orders),
        }),
    )?;
    for (index, order) in orders.iter().enumerate() {
        host::run(
            "layer.classStyle",
            json!({
                "layer": layer,
                "index": index,
                "color": style::color_for(index, &colors),
                "label": format!("Order {order}"),
            }),
        )?;
        host::run(
            "layer.lineStrokes",
            json!({
                "layer": layer,
                "index": index,
                "strokes": [{"color": null, "width": style::width_for(*order)}],
            }),
        )?;
    }
    host::notify(&format!("{layer}: {} Strahler classes colored.", orders.len()));
    Ok(json!({"layer": layer, "classes": orders.len()}))
}

fn strahler_order(req: &Request) -> Result<Value, String> {
    let features = req.features("input")?;
    let (x, y) = parse_outlet(req.str("outlet").ok_or("'outlet' is required")?)?;
    let tolerance = req.f64("tolerance").unwrap_or(0.0001);

    let lines: Vec<Option<[strahler::Key; 2]>> = features.iter().map(endpoints).collect();
    if lines.iter().all(Option::is_none) {
        return Err(format!("No usable line segments in '{}' ({} features read).", req.str("input").unwrap_or("input"), features.len()));
    }
    // The tool gets only the selected features when the layer has a selection, so a broken-up
    // network can make a junction look like a free end. Say so when the outlet is not found.
    let outlet = strahler::outlet_key(&lines, x, y, tolerance).map_err(|e| {
        format!("{e} If the layer has a selection, only the selected features are used: clear it and run again.")
    })?;
    let orders = strahler::orders(&lines, outlet)?;

    let mut highest = 0;
    let out: Vec<Value> = features
        .iter()
        .zip(&orders)
        .map(|(f, order)| {
            let mut properties = f["properties"].clone();
            let value = order.map_or(Value::Null, |o| json!(o));
            highest = highest.max(order.unwrap_or(0));
            properties[FIELD] = value;
            geojson::feature(f["geometry"].clone(), properties)
        })
        .collect();

    let numbered = orders.iter().filter(|o| o.is_some()).count();
    let name = format!("{} strahler", req.str("input").unwrap_or("Input"));
    Ok(ToolOutput::new()
        .layer(name.trim(), geojson::collection(out))
        .message(format!("{numbered} segments ordered; highest Strahler order {highest}."))
        .into())
}

/// The layer to color: the active layer (the one selected in the Layers pane), which must have a
/// `strahler` field. With no active layer, the last layer that has one.
fn strahler_layer() -> Result<String, String> {
    let layers = host::layers()?;
    let all = layers.as_array().ok_or("RustGIS sent no layers")?;
    let has_field = |l: &Value| l["fields"].as_array().is_some_and(|fs| fs.iter().any(|f| f["name"] == FIELD));
    let name = |l: &Value| l["name"].as_str().map(str::to_string).ok_or("A layer has no name.");
    if let Some(active) = all.iter().find(|l| l["active"] == true) {
        let name = name(active)?;
        return if has_field(active) {
            Ok(name)
        } else {
            Err(format!("'{name}' has no {FIELD} field. Select the Strahler layer in the Layers pane, then run again."))
        };
    }
    let last = all.iter().filter(|l| has_field(l)).last().ok_or("No layer has an Order_Strahler field. Run Strahler Order first.")?;
    name(last).map_err(Into::into)
}

/// A table with the steps for using Strahler Order, for the **Help** tool.
fn strahler_help() -> Result<Value, String> {
    let steps = [
        ("1", "Clear the layer's selection (Select > None). With a selection, only the selected segments are used."),
        ("2", "Find the free end of the root segment (the outlet) on the map: the end where the river meets nothing else."),
        ("3", "Enter its coordinates in Outlet as latitude,longitude, for example -14.819737,-56.309678."),
        ("4", "The outlet must be within the snap tolerance (default 0.0001 degrees, about 11 m) of a free end."),
        ("5", "If the point is a junction or too far away, the error names the nearest free end. Copy it into Outlet."),
        ("6", "Run. The result is a new layer whose segments carry the order in the Order_Strahler field."),
    ];
    let rows: Vec<Value> = steps
        .iter()
        .map(|(step, text)| geojson::feature(Value::Null, json!({"step": step, "instruction": text})))
        .collect();
    Ok(ToolOutput::new().table("Strahler help", geojson::collection(rows)).into())
}

/// The first and last vertex of a single line: a `LineString`, or a `MultiLineString` with one
/// part (how shapefile polylines often arrive). `None` for multipart lines and other geometries.
fn endpoints(feature: &Value) -> Option<[strahler::Key; 2]> {
    let geometry = &feature["geometry"];
    let coords = match geometry["type"].as_str()? {
        "LineString" => geometry["coordinates"].as_array()?,
        "MultiLineString" => match geometry["coordinates"].as_array()?.as_slice() {
            [only] => only.as_array()?,
            _ => return None,
        },
        _ => return None,
    };
    let vertex = |p: &Value| Some(strahler::key(p.get(0)?.as_f64()?, p.get(1)?.as_f64()?));
    Some([vertex(coords.first()?)?, vertex(coords.last()?)?])
}

/// Reads an outlet written as `latitude,longitude` (the RustGIS standard), returned as `(x, y)`
/// = `(longitude, latitude)` in degrees, the order of the features' coordinates.
fn parse_outlet(text: &str) -> Result<(f64, f64), String> {
    let (lat, lon) = text.split_once(',').ok_or("'outlet' must be written as latitude,longitude")?;
    let parse = |s: &str| s.trim().parse::<f64>().map_err(|_| format!("'outlet' has an invalid number: {s}"));
    Ok((parse(lon)?, parse(lat)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_outlet() {
        assert_eq!(parse_outlet("2, -1.5"), Ok((-1.5, 2.0)));
        assert!(parse_outlet("12").is_err());
        assert!(parse_outlet("a,2").is_err());
    }

    #[test]
    fn takes_the_ends_of_a_line_only() {
        let line = json!({"geometry": {"type": "LineString", "coordinates": [[0, 0], [1, 1], [2, 0]]}});
        assert_eq!(endpoints(&line), Some([strahler::key(0.0, 0.0), strahler::key(2.0, 0.0)]));
        let single_part = json!({"geometry": {"type": "MultiLineString", "coordinates": [[[0, 0], [2, 0]]]}});
        assert_eq!(endpoints(&single_part), Some([strahler::key(0.0, 0.0), strahler::key(2.0, 0.0)]));
        let multipart = json!({"geometry": {"type": "MultiLineString", "coordinates": [[[0, 0], [1, 1]], [[2, 2], [3, 3]]]}});
        assert_eq!(endpoints(&multipart), None);
    }
}
