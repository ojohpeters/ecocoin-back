use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub points: i32,
    pub description: Option<String>,
}
