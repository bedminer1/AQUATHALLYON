use serde::{ Serialize, Deserialize };
use teloxide::types::{InlineKeyboardButton};
use std::sync::{Arc, RwLock};
use libsql::Builder;
use std::collections::HashMap;

#[derive(Clone)]
pub struct AppState {
    pub sync_state: SharedState,
    pub db: libsql::Connection,
}

impl AppState {
    pub async fn new(initial: WeeklyAttendance) -> Self {
        let url = std::env::var("DATABASE_URL").expect("URL missing");
        let token = std::env::var("DATABASE_AUTH_TOKEN").expect("Token missing");

        let db = Builder::new_remote(url, token)
            .build()
            .await
            .expect("Failed to connect to Turso");

        let conn = db.connect().expect("Failed to connect");

        AppState {
            sync_state: SharedState::new(initial),
            db: conn,
        }
    }
}

// User Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub alias: String,
}

// Link bw user and session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub user_id: u64,
    #[serde(default)]
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSession {
    pub id: u8,
    pub activity: String,
    pub location: String,
    pub day: String,
    pub attendees: Vec<Attendee>,
    pub time: String,
}

impl TrainingSession {
    pub fn make_button(&self, week_type: &str) -> InlineKeyboardButton {
        let label = format!("{}: {} @ {}", self.day, self.activity, self.location);
        // week_type is 'c' (current) or 'n' (next)
        InlineKeyboardButton::callback(label, format!("checkin_{}_{}", week_type, self.id))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekData {
    pub start_date: String,
    pub end_date: String,
    pub sessions: Vec<TrainingSession>,
}

impl WeekData {
    pub fn get_session_mut(&mut self, session_id: u8) -> Option<&mut TrainingSession> {
        self.sessions.iter_mut().find(|s| s.id == session_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyAttendance {
    pub current: WeekData,
    pub next: WeekData,
    pub user_registry: HashMap<u64, UserProfile>,
}

#[derive(Clone)]
pub struct SharedState(pub Arc<RwLock<WeeklyAttendance>>);

impl SharedState {
    pub fn new(initial: WeeklyAttendance) -> Self {
        Self(Arc::new(RwLock::new(initial)))
    }

    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, WeeklyAttendance> {
        self.0.read().expect("Lock poisoned")
    }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, WeeklyAttendance> {
        self.0.write().expect("Lock poisoned")
    }
}
