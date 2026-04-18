use textquest_web_sdk::Client;

#[tokio::main]
async fn main() {
    // Create a client with API token authentication
    let api_token = std::env::var("TEXTQUEST_API_TOKEN")
        .unwrap_or_else(|_| "your-api-token-here".to_string());

    let client = Client::with_token("http://localhost:3000", Some(api_token));

    // Now all requests will include the X-API-Token header
    println!("Testing authenticated requests...");

    match client.health().await {
        Ok(health) => {
            println!("✓ Authenticated request succeeded");
            println!("  Status: {}", health.status);
        }
        Err(e) => eprintln!("✗ Request failed: {}", e),
    }

    // Get character configurations (requires auth)
    match client.list_character_configs().await {
        Ok(configs) => {
            println!("✓ Retrieved {} character configurations", configs.len());
            for config in configs {
                println!("  - {}", config.character_name);
            }
        }
        Err(e) => eprintln!("✗ Failed to fetch character configs: {}", e),
    }
}
