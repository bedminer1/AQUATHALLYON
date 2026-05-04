use teloxide::{prelude::*, types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode}, utils::command::BotCommands};
use chrono::{Duration, Local, Datelike, NaiveDate};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio_cron_scheduler::{Job, JobScheduler};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserProfile { alias: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Attendee { user_id: u64, #[serde(default)] cancelled: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrainingSession {
    id: u8,
    text: String,
    attendees: Vec<Attendee>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WeekData {
    start_date: String,
    end_date: String,
    sessions: Vec<TrainingSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WeeklyAttendance {
    current: WeekData,
    next: WeekData,
    user_registry: HashMap<u64, UserProfile>,
}

#[derive(Clone)]
struct AppState(Arc<RwLock<WeeklyAttendance>>);

impl AppState {
    fn load() -> Self {
        if let Ok(data) = std::fs::read_to_string("state.json") {
            if let Ok(state) = serde_json::from_str(&data) {
                return Self(Arc::new(RwLock::new(state)));
            }
        }
        let now = Local::now().date_naive();
        let days_from_monday = now.weekday().num_days_from_monday();
        let curr_monday = now - Duration::days(days_from_monday as i64);
        
        let template = if let Ok(data) = std::fs::read_to_string("sessions.txt") {
            data.lines().filter(|l| !l.trim().is_empty()).enumerate().map(|(i, l)| {
                TrainingSession {
                    id: (i + 1) as u8,
                    text: l.trim().to_string(),
                    attendees: vec![],
                }
            }).collect()
        } else {
            vec![
                TrainingSession { id: 1, text: "MON Swim 5:00 PM @ USC Pool".into(), attendees: vec![] },
                TrainingSession { id: 2, text: "TUE Run 6:00 PM @ NUS Track".into(), attendees: vec![] },
                TrainingSession { id: 3, text: "WED Swim 5:00 PM @ USC Pool".into(), attendees: vec![] },
                TrainingSession { id: 4, text: "THU Run 6:00 PM @ NUS Track".into(), attendees: vec![] },
                TrainingSession { id: 5, text: "FRI Swim 5:00 PM @ USC Pool".into(), attendees: vec![] },
                TrainingSession { id: 6, text: "SAT Bricks 8:30 AM @ Palawan Beach".into(), attendees: vec![] },
            ]
        };

        Self(Arc::new(RwLock::new(WeeklyAttendance {
            current: WeekData { start_date: curr_monday.format("%d/%m").to_string(), end_date: (curr_monday + Duration::days(7)).format("%d/%m").to_string(), sessions: template.clone() },
            next: WeekData { start_date: (curr_monday + Duration::days(7)).format("%d/%m").to_string(), end_date: (curr_monday + Duration::days(14)).format("%d/%m").to_string(), sessions: template },
            user_registry: HashMap::new(),
        })))
    }

    fn save(&self) {
        let state = self.0.read().unwrap();
        if let Ok(data) = serde_json::to_string_pretty(&*state) {
            let _ = std::fs::write("state.json", data);
        }
        let txt = state.current.sessions.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n");
        let _ = std::fs::write("sessions.txt", txt);
    }
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "snake_case")]
enum Command { Help, Show, ShowNext, Trainings, Edit(String), NewWeek }

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let state = AppState::load();
    let bot = Bot::from_env();
    let scheduler = JobScheduler::new().await.unwrap();
    let bot_c = bot.clone();
    let state_c = state.clone();
    let chat_id_env = std::env::var("CHAT_ID").ok().and_then(|id| id.parse::<i64>().ok());
    let admin_id_env = std::env::var("ADMIN_ID").ok().and_then(|id| id.parse::<u64>().ok());

    scheduler.add(Job::new_async("0 0 21 * * Sun", move |_uuid, _l| {
        let bot = bot_c.clone();
        let state = state_c.clone();
        Box::pin(async move {
            if let Some(cid) = chat_id_env {
                let (report, kb) = {
                    let mut week = state.0.write().unwrap();
                    week.current = week.next.clone();
                    let start = NaiveDate::parse_from_str(&week.current.start_date, "%d/%m").unwrap_or(Local::now().date_naive());
                    let mut next_sessions = week.current.sessions.clone();
                    for s in &mut next_sessions { s.attendees.clear(); }
                    week.next = WeekData {
                        start_date: (start + Duration::days(7)).format("%d/%m").to_string(),
                        end_date: (start + Duration::days(14)).format("%d/%m").to_string(),
                        sessions: next_sessions,
                    };
                    (generate_report(&week.current, &week.user_registry), make_keyboard(&week.current.sessions, "c"))
                };
                state.save();
                let _ = bot.send_message(ChatId(cid), format!("🔄 <b>Week Rolled!</b>\n\n{}", report)).parse_mode(ParseMode::Html).reply_markup(kb).await;
            }
        })
    }).unwrap()).await.unwrap();
    scheduler.start().await.unwrap();

    let handler = Update::filter_message().filter_command::<Command>().endpoint(move |bot: Bot, msg: Message, cmd: Command, state: AppState| async move {
        let user_id = msg.from.as_ref().map(|u| u.id.0).unwrap_or(0);
        let is_admin = admin_id_env.map_or(false, |id| id == user_id);
        let is_group = msg.chat.is_group() || msg.chat.is_supergroup();

        // Security check: Only allow admin in DMs; allow everyone in group chats
        if !is_group && !is_admin {
            return respond(());
        }

        match cmd {
            Command::Help => { bot.send_message(msg.chat.id, "/show, /show_next, /trainings, /edit, /new_week").await?; }
            Command::Show => {
                let (r, k) = { let w = state.0.read().unwrap(); (generate_report(&w.current, &w.user_registry), make_keyboard(&w.current.sessions, "c")) };
                bot.send_message(msg.chat.id, r).parse_mode(ParseMode::Html).reply_markup(k).await?;
            }
            Command::ShowNext => {
                let (r, k) = { let w = state.0.read().unwrap(); (generate_report(&w.next, &w.user_registry), make_keyboard(&w.next.sessions, "n")) };
                bot.send_message(msg.chat.id, r).parse_mode(ParseMode::Html).reply_markup(k).await?;
            }
            Command::Trainings => {
                if !is_admin { return respond(()); }
                let t = { let w = state.0.read().unwrap(); w.current.sessions.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n") };
                bot.send_message(msg.chat.id, format!("<pre>{}</pre>", t)).parse_mode(ParseMode::Html).await?;
            }
            Command::Edit(args) => {
                if !is_admin { return respond(()); }
                let (report, kb) = {
                    let mut w = state.0.write().unwrap();
                    let new: Vec<_> = args.lines().filter(|l| !l.trim().is_empty()).enumerate().map(|(i, l)| {
                        TrainingSession { id: (i+1) as u8, text: l.trim().to_string(), attendees: vec![] }
                    }).collect();
                    if !new.is_empty() { w.current.sessions = new.clone(); w.next.sessions = new; }
                    (generate_report(&w.current, &w.user_registry), make_keyboard(&w.current.sessions, "c"))
                };
                state.save();
                bot.send_message(msg.chat.id, report).parse_mode(ParseMode::Html).reply_markup(kb).await?;
            }
            Command::NewWeek => {
                if !is_admin { return respond(()); }
                let (report, kb) = {
                    let mut w = state.0.write().unwrap();
                    w.current = w.next.clone();
                    let start = NaiveDate::parse_from_str(&w.current.start_date, "%d/%m").unwrap_or(Local::now().date_naive());
                    let mut next_sessions = w.current.sessions.clone();
                    for s in &mut next_sessions { s.attendees.clear(); }
                    w.next = WeekData { start_date: (start + Duration::days(7)).format("%d/%m").to_string(), end_date: (start + Duration::days(14)).format("%d/%m").to_string(), sessions: next_sessions };
                    (generate_report(&w.current, &w.user_registry), make_keyboard(&w.current.sessions, "c"))
                };
                state.save();
                bot.send_message(msg.chat.id, format!("🔄 <b>Week Rolled!</b>\n\n{}", report)).parse_mode(ParseMode::Html).reply_markup(kb).await?;
            }
        }
        respond(())
    });

    let cb_handler = Update::filter_callback_query().endpoint(|bot: Bot, q: CallbackQuery, state: AppState| async move {
        if let (Some(data), Some(msg)) = (q.data, q.message.and_then(|m| match m { teloxide::types::MaybeInaccessibleMessage::Regular(r) => Some(r), _ => None })) {
            if let Some(s) = data.strip_prefix("ck_") {
                let p: Vec<_> = s.split('_').collect();
                let (w_t, s_id) = (p[0], p[1].parse::<u8>().unwrap_or(0));
                let (text, kb) = {
                    let mut week = state.0.write().unwrap();
                    week.user_registry.insert(q.from.id.0, UserProfile { alias: q.from.full_name() });
                    {
                        let wd = if w_t == "c" { &mut week.current } else { &mut week.next };
                        if let Some(sess) = wd.sessions.iter_mut().find(|s| s.id == s_id) {
                            if let Some(a) = sess.attendees.iter_mut().find(|a| a.user_id == q.from.id.0) { a.cancelled = !a.cancelled; }
                            else { sess.attendees.push(Attendee { user_id: q.from.id.0, cancelled: false }); }
                        }
                    }
                    let wd = if w_t == "c" { &week.current } else { &week.next };
                    (generate_report(wd, &week.user_registry), make_keyboard(&wd.sessions, w_t))
                };
                state.save();
                let _ = bot.edit_message_text(msg.chat.id, msg.id, text).parse_mode(ParseMode::Html).reply_markup(kb).await;
            }
        }
        let _ = bot.answer_callback_query(q.id).await;
        respond(())
    });

    Dispatcher::builder(bot, dptree::entry().branch(handler).branch(cb_handler)).dependencies(dptree::deps![state]).enable_ctrlc_handler().build().dispatch().await;
}

fn make_keyboard(sessions: &[TrainingSession], w_type: &str) -> InlineKeyboardMarkup {
    let b: Vec<Vec<InlineKeyboardButton>> = sessions.iter().map(|s| vec![InlineKeyboardButton::callback(s.text.clone(), format!("ck_{}_{}", w_type, s.id))]).collect();
    InlineKeyboardMarkup::new(b)
}

fn generate_report(wd: &WeekData, reg: &HashMap<u64, UserProfile>) -> String {
    let mut r = format!("📅 <b>Attendance {} - {}</b>\n\n", wd.start_date, wd.end_date);
    for s in &wd.sessions {
        let att: Vec<_> = s.attendees.iter().map(|a| {
            let n = reg.get(&a.user_id).map(|u| u.alias.clone()).unwrap_or("?".into());
            if a.cancelled { format!("<s>{}</s>", n) } else { n }
        }).collect();
        r.push_str(&format!("<b>{}</b> ({})\n", s.text, s.attendees.iter().filter(|a| !a.cancelled).count()));
        if att.is_empty() { r.push_str("<i>-</i>\n"); } else { r.push_str(&format!("{}\n", att.join(", "))); }
        r.push('\n');
    }
    r
}
