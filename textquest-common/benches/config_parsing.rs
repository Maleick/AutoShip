use criterion::{Criterion, black_box, criterion_group, criterion_main};

/// Benchmark simple JSON parsing (used as config alternative).
fn bench_simple_json_parse(c: &mut Criterion) {
    let json_data = r#"{"accounts":[{"name":"account1","server":"Firiona Vie","character":"MainChar","class":"WAR","group":1},{"name":"account2","server":"Firiona Vie","character":"OffChar","class":"CLR","group":1}]}"#;

    c.bench_function("simple_json_parse", |b| {
        b.iter(|| {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(black_box(json_data));
            parsed
        });
    });
}

/// Benchmark larger JSON config with 36 accounts.
fn bench_large_json_parse(c: &mut Criterion) {
    let mut json_str = String::from(r#"{"accounts":["#);

    for i in 1..=36 {
        if i > 1 {
            json_str.push(',');
        }
        json_str.push_str(&format!(
            r#"{{"name":"account{:02}","server":"Firiona Vie","character":"Char{:02}","class":"WAR","group":{}}}"#,
            i,
            i,
            (i - 1) / 6 + 1
        ));
    }
    json_str.push_str(r#"]}"#);

    c.bench_function("large_json_parse_36_accounts", |b| {
        b.iter(|| {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(black_box(&json_str));
            parsed
        });
    });
}

/// Benchmark field extraction from parsed JSON.
fn bench_json_field_access(c: &mut Criterion) {
    let json_data = r#"{"accounts":[{"name":"account1","server":"Firiona Vie","character":"MainChar","class":"WAR","group":1},{"name":"account2","server":"Firiona Vie","character":"OffChar","class":"CLR","group":1}]}"#;

    let parsed: serde_json::Value = serde_json::from_str(json_data).unwrap();

    c.bench_function("json_field_extraction", |b| {
        b.iter(|| {
            let accounts = parsed["accounts"].as_array();
            let mut names = Vec::new();

            if let Some(arr) = accounts {
                for account in arr {
                    if let Some(name) = account["name"].as_str() {
                        names.push(name);
                    }
                    if let Some(class) = account["class"].as_str() {
                        let _ = black_box(class);
                    }
                    if let Some(group) = account["group"].as_u64() {
                        let _ = black_box(group);
                    }
                }
            }
            names
        });
    });
}

/// Benchmark config deserialization into a struct (typical use case).
fn bench_config_struct_deser(c: &mut Criterion) {
    #[derive(serde::Deserialize)]
    struct SimpleAccount {
        #[serde(rename = "name")]
        _name: String,
        #[serde(rename = "server")]
        _server: String,
        #[serde(rename = "character")]
        _character: String,
        #[serde(rename = "class")]
        _class: String,
        #[serde(rename = "group")]
        _group: u32,
    }

    #[derive(serde::Deserialize)]
    struct SimpleConfig {
        #[serde(rename = "accounts")]
        _accounts: Vec<SimpleAccount>,
    }

    let json_data = r#"{"accounts":[{"name":"account1","server":"Firiona Vie","character":"MainChar","class":"WAR","group":1},{"name":"account2","server":"Firiona Vie","character":"OffChar","class":"CLR","group":1}]}"#;

    c.bench_function("config_struct_deserialize", |b| {
        b.iter(|| {
            let config: SimpleConfig =
                serde_json::from_str(black_box(json_data)).expect("deserialize config");
            let summary = config.accounts.iter().fold(0usize, |count, account| {
                black_box(account.name.as_str());
                black_box(account.server.as_str());
                black_box(account.character.as_str());
                black_box(account.class.as_str());
                black_box(account.group);
                count + 1
            });

            black_box(summary)
        });
    });
}

criterion_group!(
    benches,
    bench_simple_json_parse,
    bench_large_json_parse,
    bench_json_field_access,
    bench_config_struct_deser
);
criterion_main!(benches);
