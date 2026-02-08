use serde::{ Serialize, Deserialize };
use teloxide::types::{InlineKeyboardButton};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub telegram_id: u64,
    pub alias: String, // Their @username or a custom nickname
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSession {
    pub id: u8,
    pub activity: String,
    pub location: String,
    pub day: String,
    pub attendees: Vec<User>,
}

impl TrainingSession {
    pub fn make_button(&self) -> InlineKeyboardButton {
        let label = format!("{}: {} @ {}", self.day, self.activity, self.location);
        InlineKeyboardButton::callback(label, format!("checkin_{}", self.id))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyAttendance {
    pub start_date: String, // e.g., "2026-02-02"
    pub end_date: String,   // e.g., "2026-02-08"
    pub sessions: Vec<TrainingSession>,
}

impl WeeklyAttendance {
    pub fn get_session_mut(&mut self, session_id: u8) -> Option<&mut TrainingSession> {
        self.sessions.iter_mut().find(|s| s.id == session_id)
    }
}

#[derive(Clone)]
pub struct SharedState(pub Arc<RwLock<WeeklyAttendance>>);

impl SharedState {
    pub fn new(initial: WeeklyAttendance) -> Self {
        Self(Arc::new(RwLock::new(initial)))
    }

    // Helper to make your handlers cleaner
    // pub fn read(&self) -> std::sync::RwLockReadGuard<'_, WeeklyAttendance> {
    //     self.0.read().expect("Lock poisoned")
    // }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, WeeklyAttendance> {
        self.0.write().expect("Lock poisoned")
    }
}