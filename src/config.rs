use dotenvy::dotenv;
use std::env;

pub fn load_env() {
    dotenv().ok();
    println!("Loaded .env file");
}

pub fn get_env(key: &str) -> String {
    env::var(key).expect(&format!("Missing env var: {}", key))
}
