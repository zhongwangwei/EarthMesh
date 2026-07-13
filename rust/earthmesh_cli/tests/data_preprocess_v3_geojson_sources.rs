use std::fs;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

#[test]
fn v3_geojson_reader_summarizes_hydro_and_coast_source_layers() {
    let root = temp_root("data_preprocess_v3_geojson_sources");
    let hydro_path = root.join("merit_river_masks.geojson");
    let coast_path = root.join("merit_coast_masks.geojson");
    fs::write(
        &hydro_path,
        r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","properties":{"hydro_class":"R2"},"geometry":null},
            {"type":"Feature","properties":{"hydro_class":"ESTUARY"},"geometry":null}
          ]
        }"#,
    )
    .expect("write hydro geojson");
    fs::write(
        &coast_path,
        r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","properties":{"mask_class":"COAST_LAND"},"geometry":null},
            {"type":"Feature","properties":{"coast_class":"COAST_OCEAN"},"geometry":null}
          ]
        }"#,
    )
    .expect("write coast geojson");

    let hydro = earthmesh_cli::v3_data_source_io::read_v3_geojson_source_summary(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Hydro,
        &hydro_path,
    )
    .expect("read hydro geojson source");
    assert_eq!(
        hydro.source.kind,
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Hydro
    );
    assert_eq!(hydro.feature_count, 2);
    assert_eq!(hydro.classes, vec!["ESTUARY", "R2"]);
    assert_eq!(
        hydro.source.semantic_layers,
        vec!["river_r2", "river_r3", "estuary"]
    );

    let coast = earthmesh_cli::v3_data_source_io::read_v3_geojson_source_summary(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Coast,
        &coast_path,
    )
    .expect("read coast geojson source");
    assert_eq!(
        coast.source.kind,
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Coast
    );
    assert_eq!(coast.feature_count, 2);
    assert_eq!(coast.classes, vec!["COAST_LAND", "COAST_OCEAN"]);
    assert_eq!(
        coast.source.semantic_layers,
        vec!["coast_land", "coast_ocean", "shoreline"]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn v3_geojson_bundle_keeps_hydro_and_coast_sources_together_for_rust_state() {
    let root = temp_root("data_preprocess_v3_geojson_bundle");
    let hydro_path = root.join("cama_or_merit_river_masks.geojson");
    let coast_path = root.join("cama_or_merit_coast_masks.geojson");
    fs::write(
        &hydro_path,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"hydro_class":"R3"},"geometry":null}]}"#,
    )
    .expect("write hydro geojson");
    fs::write(
        &coast_path,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"mask_class":"COAST_LAND"},"geometry":null}]}"#,
    )
    .expect("write coast geojson");

    let bundle = earthmesh_cli::v3_data_source_io::read_v3_hydro_coast_source_bundle(
        &hydro_path,
        &coast_path,
    )
    .expect("read hydro/coast bundle");

    assert_eq!(bundle.sources.len(), 2);
    assert_eq!(
        bundle.sources[0].kind,
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Hydro
    );
    assert_eq!(
        bundle.sources[1].kind,
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Coast
    );
    assert_eq!(bundle.hydro.feature_count, 1);
    assert_eq!(bundle.coast.feature_count, 1);
    assert_eq!(bundle.total_feature_count, 2);
    assert_eq!(bundle.hydro.classes, vec!["R3"]);
    assert_eq!(bundle.coast.classes, vec!["COAST_LAND"]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn v3_geojson_hydro_summary_accepts_merit_mask_class_outputs() {
    let root = temp_root("data_preprocess_v3_geojson_merit_mask_class");
    let hydro_path = root.join("merit_river_masks.geojson");
    fs::write(
        &hydro_path,
        r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"mask_class":"R3"},"geometry":null},
          {"type":"Feature","properties":{"river_class":"R2"},"geometry":null}
        ]}"#,
    )
    .expect("write MERIT-style hydro geojson");

    let hydro = earthmesh_cli::v3_data_source_io::read_v3_geojson_source_summary(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Hydro,
        &hydro_path,
    )
    .expect("read MERIT-style hydro geojson source");

    assert_eq!(hydro.feature_count, 2);
    assert_eq!(
        hydro.classes,
        vec!["R2", "R3"],
        "hydro summary should accept the mask_class/river_class keys emitted by Rust MERIT/CaMa source exporters"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn v3_geojson_summary_ignores_collection_level_class_metadata() {
    let root = temp_root("data_preprocess_v3_geojson_feature_only_classes");
    let hydro_path = root.join("river_masks.geojson");
    fs::write(
        &hydro_path,
        r#"{
          "type":"FeatureCollection",
          "properties":{"mask_class":"COLLECTION_METADATA"},
          "features":[
            {"type":"Feature","properties":{"mask_class":"R2"},"geometry":null}
          ]
        }"#,
    )
    .expect("write hydro geojson with collection metadata");

    let hydro = earthmesh_cli::v3_data_source_io::read_v3_geojson_source_summary(
        earthmesh_cli::v3_data_source_io::V3DataSourceKind::Hydro,
        &hydro_path,
    )
    .expect("read hydro geojson source with collection metadata");

    assert_eq!(hydro.feature_count, 1);
    assert_eq!(
        hydro.classes,
        vec!["R2"],
        "summary classes must come from Feature.properties, not collection-level metadata"
    );

    let _ = fs::remove_dir_all(root);
}
