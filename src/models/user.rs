use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct UserInfo {
    pub wallet: String,
    pub total_points: i32,
    pub tasks_completed: Vec<Uuid>,
    pub referrals: i64,
}
