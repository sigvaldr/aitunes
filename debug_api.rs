use reqwest::Client;
use serde_json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    
    // Your saved credentials
    let server_url = "http://yggdrasil.sigvaldr.lol:8888";
    let username = "tunes";
    let password = "tunes4me";
    
    println!("🔍 Debugging Jellyfin API calls...");
    println!("Server: {}", server_url);
    println!("User: {}", username);
    
    // Step 1: Authenticate
    println!("\n1️⃣ Authenticating...");
    let auth_url = format!("{}/Users/authenticatebyname", server_url);
    let auth_body = serde_json::json!({
        "Username": username,
        "Pw": password
    });
    
    let response = client
        .post(&auth_url)
        .header("Content-Type", "application/json")
        .header("X-Emby-Authorization", "MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Version=\"1.0.0\"")
        .json(&auth_body)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await?;
        println!("❌ Authentication failed: {}", error_text);
        return Ok(());
    }
    
    let auth_response: serde_json::Value = response.json().await?;
    let access_token = auth_response["AccessToken"].as_str().unwrap();
    let user_id = auth_response["User"]["Id"].as_str().unwrap();
    
    println!("✅ Authentication successful!");
    println!("User ID: {}", user_id);
    
    // Step 2: Get user's libraries/views
    println!("\n2️⃣ Getting user libraries...");
    let libraries_url = format!("{}/Users/{}/Views", server_url, user_id);
    let response = client
        .get(&libraries_url)
        .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", access_token))
        .send()
        .await?;
    
    if response.status().is_success() {
        let libraries_response: serde_json::Value = response.json().await?;
        if let Some(items) = libraries_response["Items"].as_array() {
            println!("📚 Found {} libraries:", items.len());
            for library in items {
                let name = library["Name"].as_str().unwrap_or("Unknown");
                let collection_type = library["CollectionType"].as_str().unwrap_or("Unknown");
                let id = library["Id"].as_str().unwrap_or("Unknown");
                println!("  • {} (Type: {}, ID: {})", name, collection_type, id);
            }
        }
    } else {
        println!("❌ Failed to get libraries: {}", response.status());
    }
    
    // Step 3: Try to get items from the Music library specifically
    println!("\n3️⃣ Getting items from Music library...");
    let music_library_id = "7e64e319657a9516ec78490da03edccb"; // From the libraries output
    let music_items_url = format!("{}/Users/{}/Items?ParentId={}&Recursive=true&SortBy=Name", server_url, user_id, music_library_id);
    let response = client
        .get(&music_items_url)
        .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", access_token))
        .send()
        .await?;
    
    if response.status().is_success() {
        let music_response: serde_json::Value = response.json().await?;
        if let Some(items) = music_response["Items"].as_array() {
            println!("🎵 Found {} items in Music library", items.len());
            
            if items.is_empty() {
                println!("⚠️  Music library is empty!");
            } else {
                println!("📋 First 10 music items:");
                for (i, item) in items.iter().take(10).enumerate() {
                    let name = item["Name"].as_str().unwrap_or("Unknown");
                    let item_type = item["Type"].as_str().unwrap_or("Unknown");
                    println!("  {}. {} (Type: {})", i+1, name, item_type);
                }
                
                // Count by type
                let mut type_counts = std::collections::HashMap::new();
                for item in items {
                    if let Some(item_type) = item["Type"].as_str() {
                        *type_counts.entry(item_type).or_insert(0) += 1;
                    }
                }
                
                println!("\n📊 Music items by type:");
                for (item_type, count) in type_counts {
                    println!("  • {}: {}", item_type, count);
                }
            }
        } else {
            println!("⚠️  No 'Items' field in Music library response");
        }
    } else {
        println!("❌ Failed to get Music library items: {}", response.status());
    }
    
    // Step 4: Try specific audio query with limit
    println!("\n4️⃣ Trying audio-specific query with limit...");
    let audio_url = format!("{}/Users/{}/Items?Recursive=true&IncludeItemTypes=Audio&SortBy=Name&Limit=100", server_url, user_id);
    let response = client
        .get(&audio_url)
        .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", access_token))
        .send()
        .await?;
    
    if response.status().is_success() {
        let audio_response: serde_json::Value = response.json().await?;
        if let Some(items) = audio_response["Items"].as_array() {
            println!("🎵 Found {} audio items", items.len());
            
            if items.is_empty() {
                println!("⚠️  No audio items found with IncludeItemTypes=Audio");
            } else {
                for (i, item) in items.iter().take(5).enumerate() {
                    let name = item["Name"].as_str().unwrap_or("Unknown");
                    let item_type = item["Type"].as_str().unwrap_or("Unknown");
                    println!("  {}. {} (Type: {})", i+1, name, item_type);
                }
            }
        } else {
            println!("⚠️  No 'Items' field in audio response");
        }
    } else {
        println!("❌ Failed to get audio items: {}", response.status());
    }
    
    println!("\n🏁 Debug complete!");
    Ok(())
}