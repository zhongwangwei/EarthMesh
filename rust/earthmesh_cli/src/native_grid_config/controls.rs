use std::io;

use earthmesh_core::RefineConfig;

use crate::namelist_reader::namelist_assignments;

use super::{parse_namelist_bool, parse_namelist_i32, parse_namelist_string};

pub(crate) fn read_native_grid_refine_controls(contents: &str) -> io::Result<RefineConfig> {
    let mut refine = RefineConfig::default();
    for assignment in namelist_assignments(contents, "mkrefine")? {
        match assignment.field.as_str() {
            "istransition" => {
                refine.is_transition = parse_namelist_bool(&assignment.field, &assignment.value)?
            }
            "iterd" => refine.iter_d = parse_namelist_bool(&assignment.field, &assignment.value)?,
            "springglobal_type" => {
                refine.spring_global_type =
                    parse_namelist_i32(&assignment.field, &assignment.value)?
            }
            "springregional_type" => {
                refine.spring_regional_type =
                    parse_namelist_i32(&assignment.field, &assignment.value)?
            }
            "num_rc" => refine.num_rc = parse_namelist_i32(&assignment.field, &assignment.value)?,
            "set_dis_type" => {
                refine.set_dis_type = parse_namelist_string(&assignment.value);
            }
            "niter_refine" => {
                refine.niter_refine = parse_namelist_i32(&assignment.field, &assignment.value)?;
                refine.niter_refine_specified = true;
            }
            _ => {}
        }
    }
    Ok(refine)
}
