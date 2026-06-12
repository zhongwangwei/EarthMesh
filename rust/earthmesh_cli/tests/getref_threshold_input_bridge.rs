use earthmesh_cli::{
    build_getref_onelayer_threshold_inputs, build_getref_twolayer_threshold_inputs,
    AreaJudgeThreshold2D, AreaJudgeThreshold2Layer,
};

#[test]
fn threshold_input_bridge_maps_pair_flags_to_getref_mean_std_inputs() {
    let lai = AreaJudgeThreshold2D {
        name: "lai".into(),
        values: vec![vec![0.0, 0.0], vec![0.0, 3.0]],
    };
    let veg = AreaJudgeThreshold2D {
        name: "veg".into(),
        values: vec![vec![0.0, 0.0], vec![0.0, 4.0]],
    };
    let thresholds = vec![Some(lai), Some(veg)];

    let inputs = build_getref_onelayer_threshold_inputs(
        &thresholds,
        &[true, false, false, true],
        &[10.0, 20.0, 30.0, 40.0],
    )
    .expect("build one-layer inputs");

    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].name, "lai");
    assert_eq!(inputs[0].mean_threshold, Some(10.0));
    assert_eq!(inputs[0].std_threshold, None);
    assert_eq!(inputs[0].values[1][1], 3.0);
    assert_eq!(inputs[1].name, "veg");
    assert_eq!(inputs[1].mean_threshold, None);
    assert_eq!(inputs[1].std_threshold, Some(40.0));
}

#[test]
fn threshold_input_bridge_maps_two_layer_flags_to_getref_layer_thresholds() {
    let soil = AreaJudgeThreshold2Layer {
        name: "k_s".into(),
        layers: vec![
            vec![vec![0.0, 0.0], vec![0.0, 1.0]],
            vec![vec![0.0, 0.0], vec![0.0, 2.0]],
        ],
    };
    let thresholds = vec![Some(soil)];

    let inputs = build_getref_twolayer_threshold_inputs(
        &thresholds,
        &[true, true],
        &[[11.0, 12.0], [21.0, 22.0]],
    )
    .expect("build two-layer inputs");

    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "k_s");
    assert_eq!(inputs[0].mean_thresholds, Some([11.0, 12.0]));
    assert_eq!(inputs[0].std_thresholds, Some([21.0, 22.0]));
    assert_eq!(inputs[0].layers[0][1][1], 1.0);
    assert_eq!(inputs[0].layers[1][1][1], 2.0);
}
