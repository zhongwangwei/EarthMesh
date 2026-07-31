use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

const METHOD_C_BINARY_ZERO: u8 = 1;
const METHOD_C_BINARY_ONE: u8 = 2;
const METHOD_C_BINARY_BOTH: u8 = METHOD_C_BINARY_ZERO | METHOD_C_BINARY_ONE;

/// Bitset-backed extensional constraint over binary Method-C placement variables.
///
/// Rows come from the existing canonical checks or exact emitter; this type does
/// not define a second mesh-legality semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCBinaryTableConstraint {
    variables: Vec<usize>,
    row_count: usize,
    supports: Vec<[Vec<u64>; 2]>,
}

impl MethodCBinaryTableConstraint {
    pub fn from_rows(variables: Vec<usize>, rows: &[Vec<bool>]) -> io::Result<Self> {
        if variables.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C table variables must be strictly increasing",
            ));
        }
        if rows.iter().any(|row| row.len() != variables.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C table row width must match its variable count",
            ));
        }

        let word_count = rows.len().div_ceil(u64::BITS as usize);
        let mut supports = (0..variables.len())
            .map(|_| [vec![0u64; word_count], vec![0u64; word_count]])
            .collect::<Vec<_>>();
        for (row_index, row) in rows.iter().enumerate() {
            let word = row_index / u64::BITS as usize;
            let bit = row_index % u64::BITS as usize;
            for (variable, &value) in row.iter().enumerate() {
                supports[variable][usize::from(value)][word] |= 1u64 << bit;
            }
        }

        Ok(Self {
            variables,
            row_count: rows.len(),
            supports,
        })
    }

    pub fn variables(&self) -> &[usize] {
        &self.variables
    }

    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Reuse this exact relation on another equally-sized variable scope.
    pub fn rebind_variables(&self, variables: Vec<usize>) -> io::Result<Self> {
        if variables.len() != self.variables.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C rebound table scope must preserve the canonical table arity",
            ));
        }
        if variables.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C rebound table variables must be strictly increasing",
            ));
        }
        Ok(Self {
            variables,
            row_count: self.row_count,
            supports: self.supports.clone(),
        })
    }

    /// Detach this relation from mesh IDs for deterministic deduplication.
    pub fn canonical_relation(&self) -> Self {
        let mut rows = (0..self.row_count)
            .map(|row| {
                let word = row / u64::BITS as usize;
                let bit = row % u64::BITS as usize;
                self.supports
                    .iter()
                    .map(|support| support[1][word] & (1u64 << bit) != 0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        rows.sort_unstable();
        rows.dedup();
        Self::from_rows((0..self.variables.len()).collect(), &rows)
            .expect("canonical Method-C relation is reconstructed from its own valid rows")
    }

    fn all_rows(&self) -> Vec<u64> {
        let mut rows = vec![u64::MAX; self.row_count.div_ceil(u64::BITS as usize)];
        if let Some(last) = rows.last_mut() {
            let used = self.row_count % u64::BITS as usize;
            if used != 0 {
                *last = (1u64 << used) - 1;
            }
        }
        rows
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCBinaryTablePropagation {
    pub consistent: bool,
    pub rounds: usize,
    pub pruned_values: usize,
    pub residual_variable_count: usize,
    pub domains: Vec<u8>,
    pub active_row_counts: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCBinaryTableSystemAnalysis {
    pub propagation: MethodCBinaryTablePropagation,
    pub residual_components: Vec<Vec<usize>>,
    pub max_residual_component_width: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCCyclicBinaryScopeAnalysis {
    pub point_count: usize,
    pub variable_count: usize,
    pub point_variable_incidences: usize,
    pub max_point_variable_count: usize,
    pub distinct_incidence_signature_count: usize,
    pub max_incidence_signature_multiplicity: usize,
    pub best_cut_point: usize,
    pub min_linearized_frontier_width: usize,
}

impl MethodCBinaryTablePropagation {
    pub fn value(&self, variable: usize) -> Option<bool> {
        match self.domains.get(variable).copied() {
            Some(METHOD_C_BINARY_ZERO) => Some(false),
            Some(METHOD_C_BINARY_ONE) => Some(true),
            _ => None,
        }
    }
}

/// Measure the smallest conservative variable frontier obtained by cutting one
/// cyclic ordered interface and sweeping it linearly.
///
/// A variable remains live from its first to last incidence after the cut.
/// This is a diagnostic upper bound for an ordered-interface DP, not treewidth.
pub fn analyze_method_c_cyclic_binary_scope(
    point_variables: &[Vec<usize>],
) -> io::Result<MethodCCyclicBinaryScopeAnalysis> {
    if point_variables
        .iter()
        .any(|variables| variables.windows(2).any(|pair| pair[0] >= pair[1]))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C cyclic interface variables must be strictly increasing at every point",
        ));
    }

    let point_count = point_variables.len();
    let mut variable_positions = BTreeMap::<usize, Vec<usize>>::new();
    for (point, variables) in point_variables.iter().enumerate() {
        for &variable in variables {
            variable_positions.entry(variable).or_default().push(point);
        }
    }
    let point_variable_incidences = variable_positions.values().map(Vec::len).sum();
    let max_point_variable_count = point_variables.iter().map(Vec::len).max().unwrap_or(0);
    let mut incidence_signatures = BTreeMap::<Vec<usize>, usize>::new();
    for positions in variable_positions.values() {
        *incidence_signatures.entry(positions.clone()).or_default() += 1;
    }
    let distinct_incidence_signature_count = incidence_signatures.len();
    let max_incidence_signature_multiplicity =
        incidence_signatures.values().copied().max().unwrap_or(0);
    if point_count == 0 || variable_positions.is_empty() {
        return Ok(MethodCCyclicBinaryScopeAnalysis {
            point_count,
            variable_count: variable_positions.len(),
            point_variable_incidences,
            max_point_variable_count,
            distinct_incidence_signature_count,
            max_incidence_signature_multiplicity,
            best_cut_point: 0,
            min_linearized_frontier_width: 0,
        });
    }

    let mut best_cut_point = 0usize;
    let mut min_linearized_frontier_width = usize::MAX;
    for cut in 0..point_count {
        let mut frontier = vec![0usize; point_count];
        for positions in variable_positions.values() {
            let mut first = point_count;
            let mut last = 0usize;
            for &point in positions {
                let linear = (point + point_count - cut) % point_count;
                first = first.min(linear);
                last = last.max(linear);
            }
            for width in &mut frontier[first..=last] {
                *width += 1;
            }
        }
        let width = frontier.into_iter().max().unwrap_or(0);
        if width < min_linearized_frontier_width {
            min_linearized_frontier_width = width;
            best_cut_point = cut;
        }
    }

    Ok(MethodCCyclicBinaryScopeAnalysis {
        point_count,
        variable_count: variable_positions.len(),
        point_variable_incidences,
        max_point_variable_count,
        distinct_incidence_signature_count,
        max_incidence_signature_multiplicity,
        best_cut_point,
        min_linearized_frontier_width,
    })
}

pub(crate) fn count_method_c_u8_union_states(masks: &[u8]) -> usize {
    let masks = masks.iter().copied().collect::<BTreeSet<_>>();
    let mut states = BTreeSet::from([0u8]);
    for mask in masks {
        let next = states.iter().map(|state| state | mask).collect::<Vec<_>>();
        states.extend(next);
    }
    states.len()
}

/// Enforce generalized arc consistency across binary extensional constraints.
///
/// `initial_domains` uses bit 0 for `false` and bit 1 for `true`; pass `3` for
/// an unconstrained variable.
pub fn propagate_method_c_binary_tables(
    variable_count: usize,
    constraints: &[MethodCBinaryTableConstraint],
    initial_domains: Option<&[u8]>,
) -> io::Result<MethodCBinaryTablePropagation> {
    let mut domains = initial_domains
        .map(<[u8]>::to_vec)
        .unwrap_or_else(|| vec![METHOD_C_BINARY_BOTH; variable_count]);
    if domains.len() != variable_count
        || domains
            .iter()
            .any(|domain| *domain == 0 || *domain & !METHOD_C_BINARY_BOTH != 0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C binary domains must contain one non-empty 2-bit domain per variable",
        ));
    }
    for constraint in constraints {
        if constraint
            .variables
            .iter()
            .any(|&variable| variable >= variable_count)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C table variable is outside the shared domain array",
            ));
        }
    }

    let mut active_rows = constraints
        .iter()
        .map(MethodCBinaryTableConstraint::all_rows)
        .collect::<Vec<_>>();
    let mut rounds = 0usize;
    let mut pruned_values = 0usize;

    loop {
        rounds += 1;
        let mut changed = false;
        for (constraint_index, constraint) in constraints.iter().enumerate() {
            let active = &mut active_rows[constraint_index];
            for (local_variable, &variable) in constraint.variables.iter().enumerate() {
                let domain = domains[variable];
                for (word, active_word) in active.iter_mut().enumerate() {
                    let mut allowed = 0u64;
                    if domain & METHOD_C_BINARY_ZERO != 0 {
                        allowed |= constraint.supports[local_variable][0][word];
                    }
                    if domain & METHOD_C_BINARY_ONE != 0 {
                        allowed |= constraint.supports[local_variable][1][word];
                    }
                    *active_word &= allowed;
                }
            }
            if active.iter().all(|word| *word == 0) {
                return Ok(MethodCBinaryTablePropagation {
                    consistent: false,
                    rounds,
                    pruned_values,
                    residual_variable_count: domains
                        .iter()
                        .filter(|&&domain| domain == METHOD_C_BINARY_BOTH)
                        .count(),
                    domains,
                    active_row_counts: active_rows
                        .iter()
                        .map(|rows| rows.iter().map(|word| word.count_ones() as usize).sum())
                        .collect(),
                });
            }

            for (local_variable, &variable) in constraint.variables.iter().enumerate() {
                let previous = domains[variable];
                let mut supported = 0u8;
                for value in 0..=1 {
                    if previous & (1u8 << value) != 0
                        && active
                            .iter()
                            .zip(&constraint.supports[local_variable][value])
                            .any(|(active, support)| active & support != 0)
                    {
                        supported |= 1u8 << value;
                    }
                }
                if supported == 0 {
                    return Ok(MethodCBinaryTablePropagation {
                        consistent: false,
                        rounds,
                        pruned_values,
                        residual_variable_count: domains
                            .iter()
                            .filter(|&&domain| domain == METHOD_C_BINARY_BOTH)
                            .count(),
                        domains,
                        active_row_counts: active_rows
                            .iter()
                            .map(|rows| rows.iter().map(|word| word.count_ones() as usize).sum())
                            .collect(),
                    });
                }
                if supported != previous {
                    pruned_values += (previous.count_ones() - supported.count_ones()) as usize;
                    domains[variable] = supported;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    Ok(MethodCBinaryTablePropagation {
        consistent: true,
        rounds,
        pruned_values,
        residual_variable_count: domains
            .iter()
            .filter(|&&domain| domain == METHOD_C_BINARY_BOTH)
            .count(),
        domains,
        active_row_counts: active_rows
            .iter()
            .map(|rows| rows.iter().map(|word| word.count_ones() as usize).sum())
            .collect(),
    })
}

/// Propagate a table system and conservatively partition unresolved variables
/// by shared table scope.
pub fn analyze_method_c_binary_table_system(
    variable_count: usize,
    constraints: &[MethodCBinaryTableConstraint],
    initial_domains: Option<&[u8]>,
) -> io::Result<MethodCBinaryTableSystemAnalysis> {
    let propagation =
        propagate_method_c_binary_tables(variable_count, constraints, initial_domains)?;
    if !propagation.consistent {
        return Ok(MethodCBinaryTableSystemAnalysis {
            propagation,
            residual_components: Vec::new(),
            max_residual_component_width: 0,
        });
    }

    let unresolved = propagation
        .domains
        .iter()
        .map(|&domain| domain == METHOD_C_BINARY_BOTH)
        .collect::<Vec<_>>();
    let mut adjacency = vec![Vec::new(); variable_count];
    for constraint in constraints {
        let variables = constraint
            .variables
            .iter()
            .copied()
            .filter(|&variable| unresolved[variable])
            .collect::<Vec<_>>();
        for (index, &left) in variables.iter().enumerate() {
            for &right in &variables[index + 1..] {
                adjacency[left].push(right);
                adjacency[right].push(left);
            }
        }
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
        neighbors.dedup();
    }

    let mut visited = vec![false; variable_count];
    let mut residual_components = Vec::new();
    for start in 0..variable_count {
        if !unresolved[start] || visited[start] {
            continue;
        }
        visited[start] = true;
        let mut component = Vec::new();
        let mut stack = vec![start];
        while let Some(variable) = stack.pop() {
            component.push(variable);
            for &neighbor in &adjacency[variable] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }
        component.sort_unstable();
        residual_components.push(component);
    }
    residual_components.sort();
    let max_residual_component_width = residual_components.iter().map(Vec::len).max().unwrap_or(0);

    Ok(MethodCBinaryTableSystemAnalysis {
        propagation,
        residual_components,
        max_residual_component_width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_table_propagation_crosses_constraint_boundaries() {
        let equal = MethodCBinaryTableConstraint::from_rows(
            vec![0, 1],
            &[vec![false, false], vec![true, true]],
        )
        .expect("equality table");
        let not_equal = MethodCBinaryTableConstraint::from_rows(
            vec![1, 2],
            &[vec![false, true], vec![true, false]],
        )
        .expect("inequality table");
        let domains = [
            METHOD_C_BINARY_BOTH,
            METHOD_C_BINARY_BOTH,
            METHOD_C_BINARY_ONE,
        ];

        let propagated = propagate_method_c_binary_tables(3, &[equal, not_equal], Some(&domains))
            .expect("propagation");

        assert!(propagated.consistent);
        assert_eq!(propagated.value(0), Some(false));
        assert_eq!(propagated.value(1), Some(false));
        assert_eq!(propagated.value(2), Some(true));
        assert_eq!(propagated.residual_variable_count, 0);
    }

    #[test]
    fn binary_table_propagation_reports_domain_wipeout() {
        let only_false =
            MethodCBinaryTableConstraint::from_rows(vec![0], &[vec![false]]).expect("table");
        let domains = [METHOD_C_BINARY_ONE];

        let propagated = propagate_method_c_binary_tables(1, &[only_false], Some(&domains))
            .expect("propagation");

        assert!(!propagated.consistent);
    }

    #[test]
    fn canonical_relation_rebinds_without_changing_semantics() {
        let relation = MethodCBinaryTableConstraint::from_rows(
            vec![9, 12],
            &[vec![false, false], vec![true, true]],
        )
        .expect("table")
        .canonical_relation();
        let reordered = MethodCBinaryTableConstraint::from_rows(
            vec![2, 7],
            &[vec![true, true], vec![false, false], vec![true, true]],
        )
        .expect("reordered table")
        .canonical_relation();
        assert_eq!(relation, reordered);
        let constraints = [
            relation
                .rebind_variables(vec![0, 1])
                .expect("first occurrence"),
            relation
                .rebind_variables(vec![1, 2])
                .expect("second occurrence"),
        ];

        let propagated = propagate_method_c_binary_tables(3, &constraints, Some(&[2, 3, 3]))
            .expect("propagation");

        assert!(propagated.consistent);
        assert_eq!(
            (0..3)
                .map(|variable| propagated.value(variable))
                .collect::<Vec<_>>(),
            vec![Some(true), Some(true), Some(true)]
        );
    }

    #[test]
    fn residual_width_partitions_independent_table_occurrences() {
        let relation = MethodCBinaryTableConstraint::from_rows(
            vec![0, 1],
            &[vec![false, false], vec![true, true]],
        )
        .expect("table")
        .canonical_relation();
        let constraints = [
            relation
                .rebind_variables(vec![0, 1])
                .expect("first occurrence"),
            relation
                .rebind_variables(vec![2, 3])
                .expect("second occurrence"),
        ];

        let analysis =
            analyze_method_c_binary_table_system(5, &constraints, None).expect("analysis");

        assert!(analysis.propagation.consistent);
        assert_eq!(
            analysis.residual_components,
            vec![vec![0, 1], vec![2, 3], vec![4]]
        );
        assert_eq!(analysis.max_residual_component_width, 2);
    }

    #[test]
    fn cyclic_scope_chooses_the_cut_with_the_smallest_frontier() {
        let analysis =
            analyze_method_c_cyclic_binary_scope(&[vec![0, 1], vec![1], vec![2], vec![0, 2]])
                .expect("cyclic scope");

        assert_eq!(analysis.point_count, 4);
        assert_eq!(analysis.variable_count, 3);
        assert_eq!(analysis.point_variable_incidences, 6);
        assert_eq!(analysis.max_point_variable_count, 2);
        assert_eq!(analysis.distinct_incidence_signature_count, 3);
        assert_eq!(analysis.max_incidence_signature_multiplicity, 1);
        assert_eq!(analysis.best_cut_point, 0);
        assert_eq!(analysis.min_linearized_frontier_width, 2);
    }

    #[test]
    fn union_state_count_deduplicates_masks_and_compositions() {
        assert_eq!(count_method_c_u8_union_states(&[0b001, 0b001, 0b010]), 4);
        assert_eq!(count_method_c_u8_union_states(&[0b011, 0b001]), 3);
    }
}
