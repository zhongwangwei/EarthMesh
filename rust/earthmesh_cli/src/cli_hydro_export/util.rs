use super::usage;

pub(super) fn next_required_arg(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| usage(&format!("{flag} requires a value")))
}
