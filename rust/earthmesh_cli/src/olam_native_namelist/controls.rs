use std::io;

use earthmesh_core::RefineConfig;

use crate::olam_native_parser::olam_namelist_assignments;

use super::{parse_olam_native_bool, parse_olam_native_i32, parse_olam_native_string};

pub(crate) fn read_olam_native_refine_controls(contents: &str) -> io::Result<RefineConfig> {
    let mut refine = RefineConfig::default();
    for assignment in olam_namelist_assignments(contents, "mkrefine")? {
        match assignment.field.as_str() {
            "istransition" => {
                refine.is_transition = parse_olam_native_bool(&assignment.field, &assignment.value)?
            }
            "iterd" => {
                refine.iter_d = parse_olam_native_bool(&assignment.field, &assignment.value)?
            }
            "springglobal_type" => {
                refine.spring_global_type =
                    parse_olam_native_i32(&assignment.field, &assignment.value)?
            }
            "springregional_type" => {
                refine.spring_regional_type =
                    parse_olam_native_i32(&assignment.field, &assignment.value)?
            }
            "num_rc" => {
                refine.num_rc = parse_olam_native_i32(&assignment.field, &assignment.value)?
            }
            "set_dis_type" => {
                refine.set_dis_type = parse_olam_native_string(&assignment.value);
            }
            "niter_refine" => {
                refine.niter_refine = parse_olam_native_i32(&assignment.field, &assignment.value)?;
                refine.niter_refine_specified = true;
            }
            _ => {}
        }
    }
    Ok(refine)
}
