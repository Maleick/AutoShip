use textquest_web_sdk::BlockingClient;

fn main() {
    // Create a blocking client pointing to the local TextQuest web server
    let client = match BlockingClient::new("http://localhost:3000") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            return;
        }
    };

    // Check API health
    println!("Checking API health...");
    match client.health() {
        Ok(health) => {
            println!("✓ API is healthy");
            println!("  Status: {}", health.status);
            println!("  Version: {}", health.version);
        }
        Err(e) => {
            eprintln!("✗ Health check failed: {}", e);
            return;
        }
    }

    // List active sessions
    println!("\nFetching active sessions...");
    match client.list_sessions() {
        Ok(sessions) => {
            println!("✓ Found {} sessions", sessions.len());
            for session in sessions {
                println!("  - {}: Level {} in {} (hp: {}%)",
                    session.character_name, session.level, session.zone, session.hp_pct);
            }
        }
        Err(e) => eprintln!("✗ Failed to fetch sessions: {}", e),
    }

    // Get economy settings
    println!("\nFetching economy settings...");
    match client.get_economy_settings() {
        Ok(settings) => {
            println!("✓ Economy settings:");
            println!("  Krono enabled: {}", settings.krono.enabled);
            println!("  Banking rules: {}", settings.banking_rules.len());
            println!("  Tradeskill supplies: {}", settings.tradeskill_supplies.len());
        }
        Err(e) => eprintln!("✗ Failed to fetch economy settings: {}", e),
    }

    // Get loot rules
    println!("\nFetching loot rules...");
    match client.get_loot_rules() {
        Ok(rules) => {
            println!("✓ Loot rules:");
            println!("  Master looter: {}", rules.master_looter);
            println!("  Auto loot enabled: {}", rules.auto_loot_enabled);
            println!("  Rules count: {}", rules.rules.len());
        }
        Err(e) => eprintln!("✗ Failed to fetch loot rules: {}", e),
    }

    // List soul states
    println!("\nFetching soul states...");
    match client.list_soul_states() {
        Ok(states) => {
            println!("✓ Found {} soul states", states.len());
            for state in states {
                println!("  - {}: {} (status: {})",
                    state.character_id, state.memory_usage, state.status);
            }
        }
        Err(e) => eprintln!("✗ Failed to fetch soul states: {}", e),
    }

    // List vendor routes
    println!("\nFetching vendor routes...");
    match client.list_vendor_routes() {
        Ok(routes) => {
            println!("✓ Found {} vendor routes", routes.len());
            for route in routes {
                println!("  - {}: {} in {} (enabled: {})", route.id, route.npc_name, route.zone, route.enabled);
            }
        }
        Err(e) => eprintln!("✗ Failed to fetch vendor routes: {}", e),
    }

    println!("\nExample completed!");
}
