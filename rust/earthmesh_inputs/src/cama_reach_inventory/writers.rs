use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::cama_binary_io::{
    CamaReachClassificationThresholds, CamaReachInventoryGeoJsonWriteReport,
    CamaReachInventoryJsonlWriteReport, CamaReachInventoryReport, CamaReachRecord,
};
use crate::{json_escape_string, json_number, json_string_array};

use super::classify::classify_cama_reach_record;

/// Write CaMa reach inventory records as hydro source JSON Lines.
pub fn write_cama_reach_inventory_jsonl(
    inventory: &CamaReachInventoryReport,
    output: impl AsRef<Path>,
) -> io::Result<CamaReachInventoryJsonlWriteReport> {
    let output = output.as_ref().to_path_buf();
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut handle = fs::File::create(&output)?;
    for record in &inventory.records {
        writeln!(handle, "{}", cama_reach_record_json(record)?)?;
    }
    Ok(CamaReachInventoryJsonlWriteReport {
        output,
        record_count: inventory.records.len(),
    })
}

/// Write CaMa reach inventory records as a GeoJSON point FeatureCollection.
pub fn write_cama_reach_inventory_point_geojson(
    inventory: &CamaReachInventoryReport,
    output: impl AsRef<Path>,
) -> io::Result<CamaReachInventoryGeoJsonWriteReport> {
    let output = output.as_ref().to_path_buf();
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut handle = fs::File::create(&output)?;
    write!(handle, "{{\"type\":\"FeatureCollection\",\"features\":[")?;
    for (index, record) in inventory.records.iter().enumerate() {
        if index > 0 {
            write!(handle, ",")?;
        }
        write!(
            handle,
            "{{\"type\":\"Feature\",\"geometry\":{{\"type\":\"Point\",\"coordinates\":[{},{}]}},\"properties\":{}}}",
            json_number(record.lon),
            json_number(record.lat),
            cama_reach_record_json(record)?,
        )?;
    }
    writeln!(handle, "]}}")?;
    Ok(CamaReachInventoryGeoJsonWriteReport {
        output,
        feature_count: inventory.records.len(),
    })
}

fn cama_reach_record_json(record: &CamaReachRecord) -> io::Result<String> {
    let classification =
        classify_cama_reach_record(record, CamaReachClassificationThresholds::default())?;
    Ok(format!(
        "{{\"class_reasons\":{},\"downstream_x\":{},\"downstream_y\":{},\"effective_width_m\":{},\"floodplain_width_m\":{},\"is_estuary\":{},\"lat\":{},\"lon\":{},\"reach_id\":\"{}\",\"river_class\":\"{}\",\"river_length_m\":{},\"source\":\"cama_reach_inventory\",\"target_dx_km\":{},\"upstream_area_km2\":{},\"width_m\":{},\"x_index\":{},\"y_index\":{}}}",
        json_string_array(&classification.reasons),
        record.downstream_x,
        record.downstream_y,
        json_number(classification.effective_width_m),
        json_number(record.floodplain_width_m),
        record.is_estuary,
        json_number(record.lat),
        json_number(record.lon),
        json_escape_string(&record.reach_id),
        json_escape_string(&classification.river_class),
        json_number(record.river_length_m),
        json_number(record.target_dx_km),
        json_number(record.upstream_area_km2),
        json_number(record.width_m),
        record.x_index,
        record.y_index,
    ))
}
