use eqdiff::{
    FunctionBytes, RttiClass, VTable, byte_similarity, extract_rtti_type_descriptors_from_rdata,
    match_classes_by_rtti_name, match_vtable_slots_by_index, parse_vtable_slots,
    rank_candidates_by_byte_similarity,
};

#[test]
fn byte_similarity_ignores_displacement_and_immediate_bytes() {
    let old = FunctionBytes::new(
        0x1000,
        vec![
            0x48, 0x8d, 0x05, 0xf9, 0x0f, 0x00, 0x00, 0xb8, 0x34, 0x12, 0x00, 0x00, 0xc3,
        ],
    );
    let new = FunctionBytes::new(
        0x2000,
        vec![
            0x48, 0x8d, 0x05, 0xa1, 0x2b, 0x00, 0x00, 0xb8, 0x78, 0x56, 0x00, 0x00, 0xc3,
        ],
    );

    let score = byte_similarity(&old, &new, 32);

    assert_eq!(score, 1.0);
}

#[test]
fn byte_similarity_scores_matching_non_wildcard_bytes() {
    let old = FunctionBytes::new(0x1000, vec![0x55, 0x48, 0x89, 0xe5, 0x90, 0xc3]);
    let new = FunctionBytes::new(0x2000, vec![0x55, 0x48, 0x89, 0xec, 0xcc, 0xc3]);

    let score = byte_similarity(&old, &new, 32);

    assert!((score - (4.0 / 6.0)).abs() < f64::EPSILON);
}

#[test]
fn candidates_are_ranked_by_byte_similarity_as_tiebreaker() {
    let old = FunctionBytes::new(0x1000, vec![0x55, 0x48, 0x89, 0xe5, 0x90, 0xc3]);
    let candidates = vec![
        FunctionBytes::new(0x3000, vec![0x40, 0x48, 0x89, 0xec, 0xcc, 0xc3]),
        FunctionBytes::new(0x2000, vec![0x55, 0x48, 0x89, 0xe5, 0xcc, 0xc3]),
    ];

    let ranked = rank_candidates_by_byte_similarity(&old, &candidates, 32);

    assert_eq!(ranked[0].function_rva, 0x2000);
    assert!(ranked[0].score > ranked[1].score);
}

#[test]
fn rtti_classes_match_by_type_descriptor_name() {
    let old = vec![
        RttiClass::new(".?AVCPlayer@@".to_owned(), 0x5000, 0x6000),
        RttiClass::new(".?AVCSpawn@@".to_owned(), 0x5100, 0x6100),
    ];
    let new = vec![
        RttiClass::new(".?AVCSpawn@@".to_owned(), 0x7100, 0x8100),
        RttiClass::new(".?AVCPlayer@@".to_owned(), 0x7000, 0x8000),
    ];

    let matches = match_classes_by_rtti_name(&old, &new);

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].old_type_descriptor_rva, 0x5000);
    assert_eq!(matches[0].new_type_descriptor_rva, 0x7000);
    assert_eq!(matches[1].old_type_descriptor_rva, 0x5100);
    assert_eq!(matches[1].new_type_descriptor_rva, 0x7100);
}

#[test]
fn vtable_slots_match_by_index_for_same_rtti_class() {
    let old = VTable::new(
        ".?AVCPlayer@@".to_owned(),
        0x6000,
        vec![0x1100, 0x1200, 0x1300],
    );
    let new = VTable::new(
        ".?AVCPlayer@@".to_owned(),
        0x8000,
        vec![0x2100, 0x2200, 0x2300, 0x2400],
    );

    let matches = match_vtable_slots_by_index(&old, &new);

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[2].slot_index, 2);
    assert_eq!(matches[2].old_function_rva, 0x1300);
    assert_eq!(matches[2].new_function_rva, 0x2300);
}

#[test]
fn rtti_type_descriptors_are_extracted_from_rdata_names() {
    let mut rdata = vec![0u8; 0x80];
    rdata[0x10..0x1d].copy_from_slice(b".?AVCPlayer@@");
    rdata[0x1d] = 0;
    rdata[0x40..0x4c].copy_from_slice(b".?AVCSpawn@@");
    rdata[0x4c] = 0;

    let descriptors = extract_rtti_type_descriptors_from_rdata(&rdata, 0x3000);

    assert_eq!(descriptors.len(), 2);
    assert_eq!(descriptors[0].name, ".?AVCPlayer@@");
    assert_eq!(descriptors[0].type_descriptor_rva, 0x3000);
    assert_eq!(descriptors[1].name, ".?AVCSpawn@@");
    assert_eq!(descriptors[1].type_descriptor_rva, 0x3030);
}

#[test]
fn vtable_slots_stop_at_first_non_text_pointer() {
    let mut rdata = vec![0u8; 0x40];
    write_u64(&mut rdata, 0x08, 0x140001100);
    write_u64(&mut rdata, 0x10, 0x140001200);
    write_u64(&mut rdata, 0x18, 0x140050000);

    let slots = parse_vtable_slots(&rdata, 0x3000, 0x3008, 0x140000000, 0x1000..0x2000);

    assert_eq!(slots, vec![0x1100, 0x1200]);
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
