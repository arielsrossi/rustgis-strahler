//! Strahler: the Strahler order of each segment of a line network.
//!
//! Port of the QGIS plugin in github.com/LucasDaniel28/Strahler3.0 (originally by Alexandre
//! Delahaye). The **Strahler Order** tool reads a line layer and returns a new layer whose
//! segments carry their order in a field. The outlet is the free end of the root segment, given
//! as `lat,lon` in degrees (the RustGIS standard; features are in WGS 84); the walk goes upstream
//! from there.
//!
//! The tool gets only the selected features when a layer has a selection, so clear the selection
//! before running it.

mod strahler;

use rustgis_plugin::{Kind, Request, ToolOutput, Value, geojson, json};

fn handle(req: Request) -> Result<Value, String> {
    match (req.kind, req.id.as_str()) {
        (Kind::Tool, "strahler-order") => strahler_order(&req),
        (_, other) => Err(format!("Strahler has no '{other}'")),
    }
}

rustgis_plugin::export!(handle);

fn strahler_order(req: &Request) -> Result<Value, String> {
    let features = req.features("input")?;
    let (x, y) = parse_outlet(req.str("outlet").ok_or("'outlet' is required")?)?;
    let tolerance = req.f64("tolerance").unwrap_or(0.0001);
    let field = req.str("field").filter(|f| !f.is_empty()).unwrap_or("strahler");

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
            properties[field] = value;
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
