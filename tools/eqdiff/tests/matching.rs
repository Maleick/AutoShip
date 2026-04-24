use eqdiff::{
    ExtractedString, StringFunctionRef, StringRefMatch, apply_matches_to_offsets_json,
    match_functions_by_string_references, string_function_refs_from_xrefs,
};
use serde_json::json;

#[test]
fn string_reference_matching_maps_functions_with_unique_shared_strings() {
    let old_refs = vec![
        StringFunctionRef::new(0x1000, "CastSpell failed"),
        StringFunctionRef::new(0x1100, "DoAttack failed"),
    ];
    let new_refs = vec![
        StringFunctionRef::new(0x3000, "CastSpell failed"),
        StringFunctionRef::new(0x3300, "DoAttack failed"),
    ];

    let matches = match_functions_by_string_references(&old_refs, &new_refs);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].old_rva, 0x1000);
    assert_eq!(matches[0].new_rva, 0x3000);
    assert_eq!(matches[0].confidence, 1.0);
    assert_eq!(matches[0].matched_strings, vec!["CastSpell failed"]);
    assert_eq!(matches[1].old_rva, 0x1100);
    assert_eq!(matches[1].new_rva, 0x3300);
}

#[test]
fn string_reference_matching_uses_shared_string_count_as_tiebreaker() {
    let old_refs = vec![
        StringFunctionRef::new(0x1000, "inventory full"),
        StringFunctionRef::new(0x1000, "cannot use item"),
    ];
    let new_refs = vec![
        StringFunctionRef::new(0x3000, "inventory full"),
        StringFunctionRef::new(0x4000, "inventory full"),
        StringFunctionRef::new(0x4000, "cannot use item"),
    ];

    let matches = match_functions_by_string_references(&old_refs, &new_refs);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].old_rva, 0x1000);
    assert_eq!(matches[0].new_rva, 0x4000);
    assert_eq!(matches[0].confidence, 1.0);
    assert_eq!(
        matches[0].matched_strings,
        vec!["cannot use item", "inventory full"]
    );
}

#[test]
fn offsets_json_update_rebases_matched_function_addresses_and_preserves_layouts() {
    let offsets = json!({
        "client_date": "20260415",
        "eq_preferred_base": 0x140000000u64,
        "globals": {
            "pinstLocalPlayer": 0x140005000u64
        },
        "player_base": {
            "name": 180
        },
        "player_zone": {},
        "spawn_manager": {},
        "functions": {
            "castSpell": 0x140001000u64,
            "doAttack": 0x140009999u64
        }
    });
    let matches = match_functions_by_string_references(
        &[StringFunctionRef::new(0x1000, "CastSpell failed")],
        &[StringFunctionRef::new(0x3000, "CastSpell failed")],
    );

    let updated = apply_matches_to_offsets_json(offsets, 0x140000000, 0x140100000, &matches)
        .expect("offsets json should update");

    assert_eq!(updated["eq_preferred_base"], json!(0x140100000u64));
    assert_eq!(updated["functions"]["castSpell"], json!(0x140103000u64));
    assert_eq!(updated["functions"]["doAttack"], json!(0x140009999u64));
    assert_eq!(
        updated["globals"]["pinstLocalPlayer"],
        json!(0x140005000u64)
    );
    assert_eq!(updated["player_base"]["name"], json!(180));
}

#[test]
fn xref_matches_flatten_to_function_string_evidence() {
    let refs = string_function_refs_from_xrefs(&[StringRefMatch {
        string: ExtractedString {
            rva: 0x2000,
            value: "You cannot cast that now".to_string(),
        },
        referencing_rvas: vec![0x1000, 0x1010],
    }]);

    assert_eq!(
        refs,
        vec![
            StringFunctionRef::new(0x1000, "You cannot cast that now"),
            StringFunctionRef::new(0x1010, "You cannot cast that now"),
        ]
    );
}
