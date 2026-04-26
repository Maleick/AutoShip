use textquest_common::combat::CastResult;

/// Parse chat/system messages to extract cast result codes.
/// Maps EQ chat patterns to CastResult enum variants.
pub fn parse_cast_result(text: &str) -> Option<CastResult> {
    let lower = text.to_lowercase();

    // Success patterns
    if lower.contains("cast a spell") && lower.contains("target") {
        return Some(CastResult::Success);
    }

    // Fizzle patterns
    if lower.contains("fizzle") {
        return Some(CastResult::Fizzled);
    }

    // Collapse patterns
    if lower.contains("collapse") || lower.contains("gate collapsed") {
        return Some(CastResult::Collapsed);
    }

    // Resist patterns
    if lower.contains("resist") {
        return Some(CastResult::Resisted);
    }

    // Immune patterns
    if lower.contains("immune") {
        return Some(CastResult::Immune);
    }

    // Interrupt patterns
    if lower.contains("interrupt") || lower.contains("interrupted") {
        return Some(CastResult::Interrupted);
    }

    // Out of mana
    if lower.contains("not enough mana") || lower.contains("insufficient mana") {
        return Some(CastResult::OutOfMana);
    }

    // Out of range
    if lower.contains("out of range") {
        return Some(CastResult::OutOfRange);
    }

    // Line of sight
    if lower.contains("can't see")
        || lower.contains("cannot see")
        || lower.contains("line of sight")
    {
        return Some(CastResult::CannotSee);
    }

    // Stunned
    if lower.contains("stun") && lower.contains("cannot cast") {
        return Some(CastResult::Stunned);
    }

    // Components
    if lower.contains("component") || lower.contains("missing component") {
        return Some(CastResult::Components);
    }

    // Standing requirement
    if lower.contains("must stand") || lower.contains("standing") {
        return Some(CastResult::Standing);
    }

    // No target
    if lower.contains("no target") || lower.contains("no valid target") {
        return Some(CastResult::NoTarget);
    }

    // Not ready
    if lower.contains("not ready") || lower.contains("gem not ready") {
        return Some(CastResult::NotReady);
    }

    // Aborted
    if lower.contains("abort") {
        return Some(CastResult::Aborted);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_success() {
        let result = parse_cast_result("You cast a spell targeted at yourself.");
        assert_eq!(result, Some(CastResult::Success));
    }

    #[test]
    fn parse_fizzle() {
        let result = parse_cast_result("Your spell fizzles.");
        assert_eq!(result, Some(CastResult::Fizzled));
    }

    #[test]
    fn parse_resist() {
        let result = parse_cast_result("Your target resists your magic.");
        assert_eq!(result, Some(CastResult::Resisted));
    }

    #[test]
    fn parse_immune() {
        let result = parse_cast_result("Your target is immune to your magic.");
        assert_eq!(result, Some(CastResult::Immune));
    }

    #[test]
    fn parse_interrupt() {
        let result = parse_cast_result("Your cast was interrupted.");
        assert_eq!(result, Some(CastResult::Interrupted));
    }

    #[test]
    fn parse_out_of_mana() {
        let result = parse_cast_result("You don't have enough mana.");
        assert_eq!(result, Some(CastResult::OutOfMana));
    }

    #[test]
    fn parse_out_of_range() {
        let result = parse_cast_result("Your target is out of range.");
        assert_eq!(result, Some(CastResult::OutOfRange));
    }

    #[test]
    fn parse_line_of_sight() {
        let result = parse_cast_result("You can't see your target.");
        assert_eq!(result, Some(CastResult::CannotSee));
    }

    #[test]
    fn parse_stunned() {
        let result = parse_cast_result("You are stunned and cannot cast.");
        assert_eq!(result, Some(CastResult::Stunned));
    }

    #[test]
    fn parse_components() {
        let result = parse_cast_result("You are missing required components.");
        assert_eq!(result, Some(CastResult::Components));
    }

    #[test]
    fn parse_standing() {
        let result = parse_cast_result("You must stand to cast.");
        assert_eq!(result, Some(CastResult::Standing));
    }

    #[test]
    fn parse_no_target() {
        let result = parse_cast_result("You must select a target.");
        assert_eq!(result, Some(CastResult::NoTarget));
    }

    #[test]
    fn parse_not_ready() {
        let result = parse_cast_result("Your gem is not ready yet.");
        assert_eq!(result, Some(CastResult::NotReady));
    }

    #[test]
    fn parse_abort() {
        let result = parse_cast_result("You abort your casting.");
        assert_eq!(result, Some(CastResult::Aborted));
    }

    #[test]
    fn parse_none() {
        let result = parse_cast_result("Random chat message with no cast result.");
        assert_eq!(result, None);
    }

    #[test]
    fn parse_case_insensitive() {
        let result = parse_cast_result("YOUR SPELL FIZZLES.");
        assert_eq!(result, Some(CastResult::Fizzled));
    }
}
