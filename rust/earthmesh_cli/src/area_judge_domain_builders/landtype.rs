use crate::AreaJudgeLandtypeClass;

/// Classify a source `landtypes_global` value using the Fortran Area_judge rule.
///
/// `MOD_Area_judge.F90` sets `seaorland(i,j)=1` exactly when
/// `landtypes_global(i,j) /= 0`; therefore river/coast source codes remain
/// binary land cells at this Area_judge stage.
pub fn classify_area_judge_landtype_fortran_indexed(landtype: i32) -> AreaJudgeLandtypeClass {
    if landtype == 0 {
        AreaJudgeLandtypeClass::Ocean
    } else {
        AreaJudgeLandtypeClass::Land
    }
}
