use textquest_common::ipc::{BazaarListing, BazaarQuery, BazaarWindowSnapshot};

/// Query passive bazaar search results from the live EQ UI.
pub fn query_bazaar_results(eq_base: u64, filter: &BazaarQuery) -> Vec<BazaarWindowSnapshot> {
    #[cfg(windows)]
    {
        unsafe { query_bazaar_results_windows(eq_base, filter) }
    }

    #[cfg(not(windows))]
    {
        let _ = (eq_base, filter);
        Vec::new()
    }
}

const MAX_BAZAAR_WINDOWS: usize = 8;
const MAX_BAZAAR_ROWS: usize = 2000;
const MAX_BAZAAR_COLUMNS: usize = 8;
const MAX_WINDOW_COUNT: u32 = 500;
const MAX_CHILD_WALK: u32 = 200;

fn window_looks_bazaar_related(window_text: Option<&str>, window_sidl_name: Option<&str>) -> bool {
    [window_text, window_sidl_name]
        .into_iter()
        .flatten()
        .map(str::to_ascii_lowercase)
        .any(|value| value.contains("bazaar") || value.contains("bzr"))
}

fn list_looks_bazaar_related(window_text: Option<&str>, window_sidl_name: Option<&str>) -> bool {
    if window_looks_bazaar_related(window_text, window_sidl_name) {
        return true;
    }

    [window_text, window_sidl_name]
        .into_iter()
        .flatten()
        .map(str::to_ascii_lowercase)
        .any(|value| {
            value.contains("result")
                || value.contains("itemlist")
                || value.contains("item_list")
                || value.contains("searchlist")
                || (value.contains("search") && value.contains("list"))
                || (value.contains("item") && value.contains("list"))
        })
}

fn parse_price_copper(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase();
    if normalized
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == ',' || ch == '_')
    {
        let digits: String = normalized
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            return None;
        }
        return digits.parse::<u64>().ok().map(|value| value * 1000);
    }

    let mut total = 0u64;
    let mut matched = false;
    for token in normalized.split_whitespace() {
        let (multiplier, raw_value) = if let Some(value) = token.strip_suffix("pp") {
            (1000u64, value)
        } else if let Some(value) = token.strip_suffix("gp") {
            (100u64, value)
        } else if let Some(value) = token.strip_suffix("sp") {
            (10u64, value)
        } else if let Some(value) = token.strip_suffix("cp") {
            (1u64, value)
        } else if let Some(value) = token.strip_suffix('p') {
            (1000u64, value)
        } else if let Some(value) = token.strip_suffix('g') {
            (100u64, value)
        } else if let Some(value) = token.strip_suffix('s') {
            (10u64, value)
        } else if let Some(value) = token.strip_suffix('c') {
            (1u64, value)
        } else {
            continue;
        };

        let digits: String = raw_value.chars().filter(|ch| ch.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }

        total = total.checked_add(digits.parse::<u64>().ok()?.checked_mul(multiplier)?)?;
        matched = true;
    }

    matched.then_some(total)
}

fn parse_quantity(text: &str) -> Option<u32> {
    let digits: String = text
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }

    let value = digits.parse::<u32>().ok()?;
    (value <= MAX_BAZAAR_ROWS as u32).then_some(value)
}

fn normalize_listing(row_index: u32, columns: Vec<String>) -> BazaarListing {
    let mut price_idx = None;
    let mut price_copper = None;
    let mut price_text = None;

    for (idx, column) in columns.iter().enumerate() {
        if let Some(parsed) = parse_price_copper(column) {
            price_idx = Some(idx);
            price_copper = Some(parsed);
            price_text = Some(column.clone());
            break;
        }
    }

    let mut quantity_idx = None;
    let mut quantity = None;
    for (idx, column) in columns.iter().enumerate().rev() {
        if Some(idx) == price_idx {
            continue;
        }
        if let Some(parsed) = parse_quantity(column) {
            quantity_idx = Some(idx);
            quantity = Some(parsed);
            break;
        }
    }

    let mut text_columns: Vec<String> = columns
        .iter()
        .enumerate()
        .filter(|(idx, value)| {
            Some(*idx) != price_idx && Some(*idx) != quantity_idx && !value.trim().is_empty()
        })
        .map(|(_, value)| value.trim().to_string())
        .collect();
    text_columns.sort_by_key(|value| std::cmp::Reverse(value.len()));

    let item_name = text_columns.first().cloned();
    let trader_name = if text_columns.len() >= 2 {
        text_columns.get(1).cloned()
    } else {
        None
    };

    BazaarListing {
        row_index,
        columns,
        item_name,
        trader_name,
        price_text,
        price_copper,
        quantity,
    }
}

fn listing_matches_query(listing: &BazaarListing, filter: &BazaarQuery) -> bool {
    filter.text_contains.as_ref().is_none_or(|needle| {
        let needle = needle.to_ascii_lowercase();
        listing
            .columns
            .iter()
            .any(|column| column.to_ascii_lowercase().contains(&needle))
    })
}

#[cfg(windows)]
fn max_rows_for_filter(filter: &BazaarQuery, row_count: usize) -> usize {
    filter
        .max_rows
        .map_or(row_count, usize::from)
        .min(row_count)
        .min(MAX_BAZAAR_ROWS)
}

#[cfg(windows)]
fn read_list_row(list_wnd: usize, row: usize) -> Vec<String> {
    let mut columns = Vec::new();
    let mut consecutive_missing = 0usize;

    for col in 0..MAX_BAZAAR_COLUMNS {
        match unsafe { crate::eq::widgets::read_list_item_text(list_wnd, row, col) } {
            Some(text) => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    consecutive_missing += 1;
                    columns.push(String::new());
                } else {
                    consecutive_missing = 0;
                    columns.push(text);
                }
            }
            None => {
                consecutive_missing += 1;
                if consecutive_missing >= 2 {
                    break;
                }
            }
        }
    }

    while columns.last().is_some_and(String::is_empty) {
        columns.pop();
    }

    columns
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn collect_bazaar_lists_for_window(
    root_wnd: usize,
    window_text: Option<String>,
    window_sidl_name: Option<String>,
    filter: &BazaarQuery,
) -> Vec<BazaarWindowSnapshot> {
    use textquest_common::offsets::{eqgame, eqmain};

    let mut snapshots = Vec::new();
    let mut stack = Vec::new();
    let mut node = *((root_wnd + eqmain::CXWND_FIRST_NODE) as *const usize);
    let mut seeded = 0u32;
    while node != 0 && seeded < MAX_CHILD_WALK {
        seeded += 1;
        stack.push(node);
        node = *((node + eqmain::CXWND_NEXT) as *const usize);
    }

    let mut walked = 0u32;
    while let Some(current) = stack.pop() {
        walked += 1;
        if walked > MAX_CHILD_WALK || snapshots.len() >= MAX_BAZAAR_WINDOWS {
            break;
        }

        let child_text = crate::eq::widgets::read_cxstr(current + eqmain::CXWND_WINDOW_TEXT);
        let child_sidl =
            crate::eq::widgets::read_cxstr(current + eqgame::CSIDL_SCREEN_WND_SIDL_TEXT);
        let mut child = *((current + eqmain::CXWND_FIRST_NODE) as *const usize);
        while child != 0 && walked + stack.len() as u32 <= MAX_CHILD_WALK {
            stack.push(child);
            child = *((child + eqmain::CXWND_NEXT) as *const usize);
        }

        if !list_looks_bazaar_related(child_text.as_deref(), child_sidl.as_deref()) {
            continue;
        }

        let row_count = unsafe { crate::eq::widgets::list_row_count(current) };
        if row_count == 0 || row_count > MAX_BAZAAR_ROWS {
            continue;
        }

        let max_rows = max_rows_for_filter(filter, row_count);
        let listings: Vec<BazaarListing> = (0..max_rows)
            .filter_map(|row| {
                let columns = read_list_row(current, row);
                if columns.is_empty() {
                    return None;
                }

                let listing = normalize_listing(row as u32, columns);
                listing_matches_query(&listing, filter).then_some(listing)
            })
            .collect();

        if listings.is_empty() {
            continue;
        }

        snapshots.push(BazaarWindowSnapshot {
            window_text: window_text.clone(),
            window_sidl_name: window_sidl_name.clone(),
            list_sidl_name: child_sidl,
            row_count: row_count as u32,
            listings,
        });
    }

    snapshots
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn query_bazaar_results_windows(
    eq_base: u64,
    filter: &BazaarQuery,
) -> Vec<BazaarWindowSnapshot> {
    use textquest_common::offsets::{PINST_CXWND_MANAGER, eqgame, eqmain, rebase};

    if eq_base == 0 {
        return Vec::new();
    }

    let Some(mgr_addr) = rebase(PINST_CXWND_MANAGER, eq_base) else {
        return Vec::new();
    };
    let mgr_ptr = *(mgr_addr as *const usize);
    if mgr_ptr == 0 {
        return Vec::new();
    }

    let array_ptr = *((mgr_ptr + eqgame::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
    let count = *((mgr_ptr + eqgame::CXWNDMGR_WINDOWS_COUNT) as *const u32);
    if array_ptr == 0 || count == 0 || count > MAX_WINDOW_COUNT {
        return Vec::new();
    }

    let mut snapshots = Vec::new();
    for idx in 0..count as usize {
        if snapshots.len() >= MAX_BAZAAR_WINDOWS {
            break;
        }

        let wnd_ptr = *((array_ptr + idx * size_of::<usize>()) as *const usize);
        if wnd_ptr == 0 || !crate::eq::widgets::is_visible(wnd_ptr) {
            continue;
        }

        let window_text = crate::eq::widgets::read_cxstr(wnd_ptr + eqmain::CXWND_WINDOW_TEXT);
        let window_sidl_name =
            crate::eq::widgets::read_cxstr(wnd_ptr + eqgame::CSIDL_SCREEN_WND_SIDL_TEXT);
        if !window_looks_bazaar_related(window_text.as_deref(), window_sidl_name.as_deref()) {
            continue;
        }

        snapshots.extend(collect_bazaar_lists_for_window(
            wnd_ptr,
            window_text,
            window_sidl_name,
            filter,
        ));
    }

    snapshots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_looks_bazaar_related_matches_known_names() {
        assert!(window_looks_bazaar_related(
            Some("Bazaar Search"),
            Some("BZR_SearchWnd")
        ));
        assert!(window_looks_bazaar_related(None, Some("bazaarsearchwnd")));
        assert!(!window_looks_bazaar_related(
            Some("Inventory"),
            Some("InventoryWindow")
        ));
    }

    #[test]
    fn parse_price_copper_supports_plain_platinum_and_denominations() {
        assert_eq!(parse_price_copper("2,000"), Some(2_000_000));
        assert_eq!(parse_price_copper("12p 3g 4s 5c"), Some(12_345));
        assert_eq!(parse_price_copper("not a price"), None);
    }

    #[test]
    fn normalize_listing_extracts_item_trader_price_and_quantity() {
        let listing = normalize_listing(
            7,
            vec![
                "Fungi Tunic".into(),
                "2,000".into(),
                "Traderbob".into(),
                "1".into(),
            ],
        );

        assert_eq!(listing.row_index, 7);
        assert_eq!(listing.item_name.as_deref(), Some("Fungi Tunic"));
        assert_eq!(listing.trader_name.as_deref(), Some("Traderbob"));
        assert_eq!(listing.price_text.as_deref(), Some("2,000"));
        assert_eq!(listing.price_copper, Some(2_000_000));
        assert_eq!(listing.quantity, Some(1));
    }

    #[test]
    fn listing_matches_query_checks_all_columns_case_insensitively() {
        let listing = BazaarListing {
            row_index: 0,
            columns: vec![
                "Fungi Tunic".into(),
                "2,000".into(),
                "Traderbob".into(),
                "1".into(),
            ],
            item_name: Some("Fungi Tunic".into()),
            trader_name: Some("Traderbob".into()),
            price_text: Some("2,000".into()),
            price_copper: Some(2_000_000),
            quantity: Some(1),
        };

        assert!(listing_matches_query(
            &listing,
            &BazaarQuery {
                text_contains: Some("traderBOB".into()),
                max_rows: None,
            }
        ));
        assert!(!listing_matches_query(
            &listing,
            &BazaarQuery {
                text_contains: Some("fungal".into()),
                max_rows: None,
            }
        ));
    }
}
