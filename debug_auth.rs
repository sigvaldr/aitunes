use reqwest::Client;
use serde_json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    
    // Replace these with your actual server details
    let server_url = "http://localhost:8096"; // Change this to your Jellyfin server URL
    let username = "your_username"; // Change this to your username
    let password = "your_password"; // Change this to your password
    
    println!("Testing Jellyfin authentication...");
    println!("Server URL: {}", server_url);
    println!("Username: {}", username);
    
    // Test 1: Check if server is reachable
    println!("\n1. Testing server connection...");
    let system_info_url = format!("{}/System/Info/Public", server_url);
    let response = client.get(&system_info_url).send().await?;
    println!("System info status: {}", response.status());
    
    if response.status().is_success() {
        let system_info: serde_json::Value = response.json().await?;
        println!("Server name: {}", system_info["ServerName"].as_str().unwrap_or("Unknown"));
        println!("Version: {}", system_info["Version"].as_str().unwrap_or("Unknown"));
    } else {
        let error_text = response.text().await?;
        println!("Error: {}", error_text);
        return Ok(());
    }
    
    // Test 2: Try authentication
    println!("\n2. Testing authentication...");
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
    
    println!("Auth status: {}", response.status());
    
    if response.status().is_success() {
        let auth_response: serde_json::Value = response.json().await?;
        println!("Authentication successful!");
        println!("Access token: {}", auth_response["AccessToken"].as_str().unwrap_or("None"));
        println!("User ID: {}", auth_response["User"]["Id"].as_str().unwrap_or("None"));
        println!("User name: {}", auth_response["User"]["Name"].as_str().unwrap_or("None"));
    } else {
        let error_text = response.text().await?;
        println!("Authentication failed: {}", error_text);
    }
    
    Ok(())
}