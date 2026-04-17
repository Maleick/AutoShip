use textquest_common::ipc::{MerchantListing, MerchantQuery, MerchantWindowSnapshot};

/// Query visible merchant window rows from the live EQ UI.
pub fn query_merchant_items(eq_base: u64, filter: &MerchantQuery) -> Vec<MerchantWindowSnapshot> {
    #[cfg(windows)]
    {
        unsafe { query_merchant_items_windows(eq_base, filter) }
    }

    #[cfg(not(windows))]
    {
        let _ = (eq_base, filter);
        Vec::new()
    }
}

const MAX_MERCHANT_WINDOWS: usize = 4;
const MAX_MERCHANT_ROWS: usize = 256;
const MAX_MERCHANT_COLUMNS: usize = 7;
const MAX_WINDOW_COUNT: u32 = 500;

fn clean_vendor_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(stripped) = trimmed.strip_prefix('>')
        && let Some(stripped) = stripped.strip_suffix('<')
    {
        let cleaned = stripped.trim();
        if !cleaned.is_empty() {
            return Some(cleaned.to_string());
        }
    }

    Some(trimmed.to_string())
}

fn parse_price_copper(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase().replace(',', "");
    if normalized
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '.')
    {
        if let Some((whole, fractional)) = normalized.split_once('.') {
            let plat = if whole.is_empty() {
                0
            } else {
                whole.parse::<u64>().ok()?
            };
            let mut fractional_digits: String = fractional
                .chars()
                .filter(|ch| ch.is_ascii_digit())
                .take(3)
                .collect();
            while fractional_digits.len() < 3 {
                fractional_digits.push('0');
            }
            let sub_plat = if fractional_digits.is_empty() {
                0
            } else {
                fractional_digits.parse::<u64>().ok()?
            };
            return plat.checked_mul(1000)?.checked_add(sub_plat);
        }

        return normalized.parse::<u64>().ok().map(|value| value * 1000);
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

fn parse_quantity(text: &str) -> (Option<u32>, bool) {
    let trimmed = text.trim();
    if trimmed == "--" {
        return (None, true);
    }

    let digits: String = trimmed.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return (None, false);
    }

    (digits.parse::<u32>().ok(), false)
}

fn normalize_listing(row_index: u32, columns: Vec<String>) -> MerchantListing {
    let item_name = columns
        .get(1)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let quantity_text = columns
        .get(2)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let price_text = columns
        .get(4)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let (quantity, infinite_quantity) = quantity_text
        .as_deref()
        .map(parse_quantity)
        .unwrap_or((None, false));
    let price_copper = price_text.as_deref().and_then(parse_price_copper);

    MerchantListing {
        row_index,
        columns,
        item_name,
        price_text,
        price_copper,
        quantity,
        quantity_text,
        infinite_quantity,
    }
}

fn listing_matches_query(listing: &MerchantListing, filter: &MerchantQuery) -> bool {
    filter.text_contains.as_ref().is_none_or(|needle| {
        let needle = needle.to_ascii_lowercase();
        listing
            .columns
            .iter()
            .any(|column| column.to_ascii_lowercase().contains(&needle))
    })
}

fn match_limit_for_filter(filter: &MerchantQuery, row_count: usize) -> usize {
    filter
        .max_rows
        .map_or(row_count, usize::from)
        .min(row_count)
        .min(MAX_MERCHANT_ROWS)
}

fn collect_matching_listings<F>(
    row_count: usize,
    filter: &MerchantQuery,
    mut read_row: F,
) -> Vec<MerchantListing>
where
    F: FnMut(usize) -> Vec<String>,
{
    let scan_limit = row_count.min(MAX_MERCHANT_ROWS);
    let match_limit = match_limit_for_filter(filter, scan_limit);
    let mut listings = Vec::new();

    for row in 0..scan_limit {
        let columns = read_row(row);
        if columns.is_empty() {
            continue;
        }

        let listing = normalize_listing(row as u32, columns);
        if !listing_matches_query(&listing, filter) {
            continue;
        }

        listings.push(listing);
        if listings.len() >= match_limit {
            break;
        }
    }

    listings
}

#[cfg(windows)]
fn read_value<T: Copy>(addr: usize) -> Option<T> {
    if !crate::hooks::game_loop::is_readable(addr, std::mem::size_of::<T>()) {
        return None;
    }

    Some(unsafe { std::ptr::read_unaligned(addr as *const T) })
}

#[cfg(windows)]
fn read_ptr(addr: usize) -> Option<usize> {
    read_value::<usize>(addr).filter(|value| *value != 0)
}

#[cfg(windows)]
fn read_list_row(list_wnd: usize, row: usize) -> Vec<String> {
    let mut columns = Vec::new();
    let mut consecutive_missing = 0usize;

    for col in 0..MAX_MERCHANT_COLUMNS {
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
                columns.push(String::new());
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
unsafe fn query_merchant_items_windows(
    eq_base: u64,
    filter: &MerchantQuery,
) -> Vec<MerchantWindowSnapshot> {
    use textquest_common::offsets::{PINST_CXWND_MANAGER, eqgame, eqmain, rebase};

    if eq_base == 0 {
        return Vec::new();
    }

    let Some(mgr_addr) = rebase(PINST_CXWND_MANAGER, eq_base) else {
        return Vec::new();
    };
    let Some(mgr_ptr) = read_ptr(mgr_addr as usize) else {
        return Vec::new();
    };

    let Some(array_ptr) = read_ptr(mgr_ptr + eqgame::CXWNDMGR_WINDOWS_ARRAY) else {
        return Vec::new();
    };
    let Some(count) = read_value::<u32>(mgr_ptr + eqgame::CXWNDMGR_WINDOWS_COUNT) else {
        return Vec::new();
    };
    if count == 0 || count > MAX_WINDOW_COUNT {
        return Vec::new();
    }

    let mut windows = Vec::new();
    for idx in 0..count as usize {
        if windows.len() >= MAX_MERCHANT_WINDOWS {
            break;
        }

        let Some(wnd_ptr) = read_ptr(array_ptr + idx * std::mem::size_of::<usize>()) else {
            continue;
        };
        if !crate::eq::widgets::is_visible(wnd_ptr) {
            continue;
        }

        let Some(purchase_page) =
            crate::eq::widgets::find_child_by_sidl_text(wnd_ptr, "MW_PurchasePage")
        else {
            continue;
        };
        let Some(item_list) =
            crate::eq::widgets::find_child_by_sidl_text(purchase_page, "MW_ItemList")
        else {
            continue;
        };

        let row_count = crate::eq::widgets::list_row_count(item_list);
        if row_count == 0 {
            continue;
        }

        let listings =
            collect_matching_listings(row_count, filter, |row| read_list_row(item_list, row));
        if listings.is_empty() {
            continue;
        }

        let window_text = crate::eq::widgets::read_cxstr(wnd_ptr + eqmain::CXWND_WINDOW_TEXT);
        let window_sidl_name =
            crate::eq::widgets::read_cxstr(wnd_ptr + eqgame::CSIDL_SCREEN_WND_SIDL_TEXT);
        let list_sidl_name =
            crate::eq::widgets::read_cxstr(item_list + eqgame::CSIDL_SCREEN_WND_SIDL_TEXT);
        let vendor_name = window_text.as_deref().and_then(clean_vendor_name);

        windows.push(MerchantWindowSnapshot {
            vendor_name,
            window_text,
            window_sidl_name,
            list_sidl_name,
            row_count: row_count as u32,
            listings,
        });
    }

    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_price_copper_supports_whole_and_decimal_platinum() {
        assert_eq!(parse_price_copper("475"), Some(475_000));
        assert_eq!(parse_price_copper("12.345"), Some(12_345));
        assert_eq!(parse_price_copper("1p 2g 3s 4c"), Some(1_234));
        assert_eq!(parse_price_copper("not a price"), None);
    }

    #[test]
    fn parse_quantity_detects_infinite_merchant_stock() {
        assert_eq!(parse_quantity("--"), (None, true));
        assert_eq!(parse_quantity("7"), (Some(7), false));
        assert_eq!(parse_quantity(""), (None, false));
    }

    #[test]
    fn normalize_listing_reads_eq_merchant_columns() {
        let listing = normalize_listing(
            4,
            vec![
                String::new(),
                "Fungi Covered Scale Tunic".into(),
                "--".into(),
                String::new(),
                "475".into(),
            ],
        );

        assert_eq!(listing.row_index, 4);
        assert_eq!(
            listing.item_name.as_deref(),
            Some("Fungi Covered Scale Tunic")
        );
        assert_eq!(listing.price_text.as_deref(), Some("475"));
        assert_eq!(listing.price_copper, Some(475_000));
        assert_eq!(listing.quantity, None);
        assert_eq!(listing.quantity_text.as_deref(), Some("--"));
        assert!(listing.infinite_quantity);
    }

    #[test]
    fn clean_vendor_name_strips_window_markup() {
        assert_eq!(
            clean_vendor_name("> Merchant_Leah <").as_deref(),
            Some("Merchant_Leah")
        );
        assert_eq!(
            clean_vendor_name("Merchant_Leah").as_deref(),
            Some("Merchant_Leah")
        );
    }

    #[test]
    fn collect_matching_listings_caps_large_merchant_windows_instead_of_skipping_them() {
        let listings = collect_matching_listings(
            MAX_MERCHANT_ROWS + 32,
            &MerchantQuery {
                text_contains: Some("fungi".into()),
                max_rows: Some(1),
            },
            |row| {
                if row == 42 {
                    vec![
                        String::new(),
                        "Fungi Covered Scale Tunic".into(),
                        "--".into(),
                        String::new(),
                        "475".into(),
                    ]
                } else {
                    Vec::new()
                }
            },
        );

        assert_eq!(listings.len(), 1);
        assert_eq!(
            listings[0].item_name.as_deref(),
            Some("Fungi Covered Scale Tunic")
        );
    }
}
