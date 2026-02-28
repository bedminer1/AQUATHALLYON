use teloxide::{
    prelude::*,
    types::{ InlineKeyboardMarkup },
    utils::command::BotCommands,
};
use chrono::{Duration, Local, Datelike};

use crate::types::*;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "snake_case")]
pub enum Command {
    #[command(description = "Show this help menu")]
    Help,


    // --- CLUB SESSIONS MANAGEMENT ---
    #[command(description = "Show current week's attendance")]
    Show,

    #[command(description = "Show next week's attendance")]
    ShowNext,
    
    #[command(description = "Roll to next week (Current = Next, Next = Reset)")]
    NewWeek,

    #[command(description = "Add session: /add (order), (day), (act), (loc), (time)")]
    Add(String),

    #[command(description = "Remove session: /delete (order)")]
    Delete(u8),

    #[command(description = "Edit session: /edit (order), (day), (act), (loc), (time)")]
    Edit(String),

    #[command(description = "Sync current week data to database")]
    Save,

    // --- PERSONAL LOGGING (FOR MEMBERS) ---
    #[command(description = "Log personal workout: /log (act), (details)")]
    Log(String),

    #[command(description = "View training history")]
    History,
}

pub async fn handle_commands(
    bot: Bot,
    state: AppState,
    msg: Message,
    cmd: Command,
) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            let help_text = "<b>🔱 Aquathallyon Bot Help</b>\n\n\
<b>Member Commands</b>\n\
/history - View your training history\n\
/log - Record a personal workout\n\n\
<b>Manage week</b>\n\
/show - View current week's attendance\n\
/show_next - View next week's attendance\n\
/new_week - Roll to next week\n\n\
<b>Edit activities</b>\n\
/add - Create a new session (both weeks)\n\
/edit - Modify a session (both weeks)\n\
/delete - Remove a session (both weeks)\n\
/save - Sync current week to database\n\n\
<i>Tip: Separate arguments with commas. \n The format is (order), (day), (activity), (location), (time)</i>";
            bot.send_message(msg.chat.id, help_text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
        Command::Show => {
            let (report, kb) = {
                let week = state.sync_state.read();
                (generate_attendance_report(&week.current, &week.user_registry), main_menu_keyboard(&week.current.sessions, "c"))
            };
            bot.send_message(msg.chat.id, report)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
        Command::ShowNext => {
            let (report, kb) = {
                let week = state.sync_state.read();
                (generate_attendance_report(&week.next, &week.user_registry), main_menu_keyboard(&week.next.sessions, "n"))
            };
            bot.send_message(msg.chat.id, report)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
        Command::NewWeek => {
            let (report, kb) = {
                let mut week = state.sync_state.write();

                // 1. Move Next to Current
                week.current = week.next.clone();

                // 2. Generate New Next Week Dates
                let next_start = chrono::NaiveDate::parse_from_str(&week.current.start_date, "%d/%m")
                    .map(|d| d.with_year(Local::now().year()).unwrap())
                    .unwrap_or_else(|_| Local::now().date_naive()) + Duration::days(7);
                let next_end = next_start + Duration::days(6);

                // 3. Reset Next Week attendees but keep structure
                let mut new_next_sessions = week.current.sessions.clone();
                for s in &mut new_next_sessions {
                    s.attendees.clear();
                }

                week.next = WeekData {
                    start_date: next_start.format("%d/%m").to_string(),
                    end_date: next_end.format("%d/%m").to_string(),
                    sessions: new_next_sessions,
                };

                (generate_attendance_report(&week.current, &week.user_registry), main_menu_keyboard(&week.current.sessions, "c"))
            };

            bot.send_message(msg.chat.id, format!("🔄 <b>Week Rolled Forward!</b>\n\n{}", report))
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
        Command::Save => {
            let week_snapshot = state.sync_state.read().clone();

            state.db.execute("DELETE FROM attendance", ()).await.unwrap();

            for session in week_snapshot.current.sessions {
                for attendee in session.attendees {
                    if !attendee.cancelled {
                        let alias = week_snapshot.user_registry
                            .get(&attendee.user_id)
                            .map(|u| u.alias.as_str())
                            .unwrap_or("Unknown");

                        state.db.execute(
                            "INSERT INTO attendance (session_id, user_id, user_alias) VALUES (?, ?, ?)",
                            libsql::params![session.id, attendee.user_id, alias],
                        ).await.unwrap();
                    }
                }
            }

            bot.send_message(msg.chat.id, "✅ Current week attendance synced to Turso!").await?;
        }
        Command::Edit(raw_args) => {
            let parts: Vec<&str> = raw_args.split(',').map(|s| s.trim()).collect();

            if parts.len() < 5 {
                bot.send_message(msg.chat.id, "❌ Format: /edit order, day, activity, location, time").await?;
                return Ok(());
            }

            let order: usize = parts[0].parse().unwrap_or(0);
            let day = parts[1].to_string();
            let activity = parts[2].to_string();
            let location = parts[3].to_string();
            let time = parts[4].to_string();

            let (report, kb, success) = {
                let mut week = state.sync_state.write();
                let index = order.saturating_sub(1);

                if index < week.current.sessions.len() {
                    // Update current
                    let s_curr = &mut week.current.sessions[index];
                    s_curr.day = day.clone();
                    s_curr.activity = activity.clone();
                    s_curr.location = location.clone();
                    s_curr.time = time.clone();

                    // Update next (keep structural sync)
                    if index < week.next.sessions.len() {
                        let s_next = &mut week.next.sessions[index];
                        s_next.day = day;
                        s_next.activity = activity;
                        s_next.location = location;
                        s_next.time = time;
                    }

                    (generate_attendance_report(&week.current, &week.user_registry), main_menu_keyboard(&week.current.sessions, "c"), true)
                } else {
                    (String::new(), InlineKeyboardMarkup::default(), false)
                }
            };

            if success {
                bot.send_message(msg.chat.id, format!("📝 <b>Session #{} updated for both weeks.</b>\n\n{}", order, report))
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(kb)
                    .await?;
            } else {
                bot.send_message(msg.chat.id, format!("⚠️ Session #{} not found.", order)).await?;
            }
        }
        Command::Add(raw_args) => {
            let parts: Vec<&str> = raw_args.split(',').map(|s| s.trim()).collect();

            if parts.len() < 5 {
                bot.send_message(msg.chat.id, "❌ Format: /add order, day, activity, location, time").await?;
                return Ok(());
            }

            let order: usize = parts[0].parse().unwrap_or(1);
            let day = parts[1].to_string();
            let activity = parts[2].to_string();
            let location = parts[3].to_string();
            let time = parts[4].to_string();

            let (report, kb) = {
                let mut week = state.sync_state.write();
                let next_id = week.current.sessions.iter().map(|s| s.id).max()
                    .max(week.next.sessions.iter().map(|s| s.id).max())
                    .unwrap_or(0) + 1;

                let new_session = TrainingSession {
                    id: next_id,
                    activity,
                    location,
                    day,
                    attendees: vec![],
                    time,
                };

                // Add to current
                if order > 0 && order <= week.current.sessions.len() {
                    week.current.sessions.insert(order - 1, new_session.clone());
                } else {
                    week.current.sessions.push(new_session.clone());
                }

                // Add to next
                if order > 0 && order <= week.next.sessions.len() {
                    week.next.sessions.insert(order - 1, new_session);
                } else {
                    week.next.sessions.push(new_session);
                }

                (generate_attendance_report(&week.current, &week.user_registry), main_menu_keyboard(&week.current.sessions, "c"))
            };

            bot.send_message(msg.chat.id, format!("➕ <b>New session added to both weeks.</b>\n\n{}", report))
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
        Command::Delete(order) => {
            let (report, kb, success) = {
                let mut week = state.sync_state.write();
                let index = (order as usize).saturating_sub(1);

                if index < week.current.sessions.len() {
                    week.current.sessions.remove(index);
                    if index < week.next.sessions.len() {
                        week.next.sessions.remove(index);
                    }
                    (generate_attendance_report(&week.current, &week.user_registry), main_menu_keyboard(&week.current.sessions, "c"), true)
                } else {
                    (String::new(), InlineKeyboardMarkup::default(), false)
                }
            };

            if success {
                bot.send_message(msg.chat.id, format!("🗑️ <b>Session #{} deleted from both weeks.</b>\n\n{}", order, report))
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(kb)
                    .await?;
            } else {
                bot.send_message(msg.chat.id, format!("⚠️ Order #{} not found.", order)).await?;
            }
        }
        Command::History => {
            let (report, kb) = {
                let week = state.sync_state.read();
                (generate_attendance_report(&week.current, &week.user_registry), main_menu_keyboard(&week.current.sessions, "c"))
            };
            bot.send_message(msg.chat.id, report)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
        Command::Log(_raw_args) => {
            let (report, kb) = {
                let week = state.sync_state.read();
                (generate_log_report(&week.current), main_menu_keyboard(&week.current.sessions, "c"))
            };
            bot.send_message(msg.chat.id, report)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
    }

    Ok(())
}

pub async fn receive_btn_press(
    bot: Bot,
    state: AppState,
    q: CallbackQuery,
) -> ResponseResult<()> {
    let display_name = q.from.full_name();
    let user_id = q.from.id.0;

    let (report_text, keyboard, success) = {
        let mut week = state.sync_state.write();

        // Format: checkin_{week_type}_{id}
        let parts: Vec<&str> = q.data.as_deref()
            .and_then(|data| data.strip_prefix("checkin_"))
            .map(|s| s.split('_').collect())
            .unwrap_or_default();

        if parts.len() == 2 {
            let week_type = parts[0]; // 'c' or 'n'
            let session_id = parts[1].parse::<u8>().ok();

            if let Some(sid) = session_id {
                week.user_registry.insert(user_id, UserProfile { alias: display_name });

                let mut success = false;
                {
                    let week_data = if week_type == "c" { &mut week.current } else { &mut week.next };
                    if let Some(session) = week_data.get_session_mut(sid) {
                        if let Some(attendee) = session.attendees.iter_mut().find(|a| a.user_id == user_id) {
                            attendee.cancelled = !attendee.cancelled;
                        } else {
                            session.attendees.push(Attendee { user_id, cancelled: false });
                        }
                        success = true;
                    }
                }

                if success {
                    let week_data = if week_type == "c" { &week.current } else { &week.next };
                    let text = generate_attendance_report(week_data, &week.user_registry);
                    let kb = main_menu_keyboard(&week_data.sessions, week_type);
                    (Some(text), Some(kb), true)
                } else {
                    (None, None, false)
                }
            } else {
                (None, None, false)
            }
        } else {
            (None, None, false)
        }
    };

    if let (Some(text), Some(kb)) = (report_text, keyboard) {
        if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = q.message {
            bot.edit_message_text(msg.chat.id, msg.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
    }

    if !success {
        bot.answer_callback_query(q.id).text("⚠️ Session not found.").show_alert(true).await?;
    } else {
        bot.answer_callback_query(q.id).await?;
    }
    Ok(())
}

fn main_menu_keyboard(sessions: &[TrainingSession], week_type: &str) -> InlineKeyboardMarkup {
    let rows = sessions
        .iter()
        .map(|s| vec![s.make_button(week_type)])
        .collect::<Vec<_>>();

    InlineKeyboardMarkup::new(rows)
}

fn generate_attendance_report(week: &WeekData, registry: &std::collections::HashMap<u64, UserProfile>) -> String {
    let header = format!("📅 <b>Attendance {} to {}</b>\n\n", week.start_date, week.end_date);

    let body = week.sessions.iter().map(|s| {
        let attendees = if s.attendees.is_empty() {
            "<i>No one yet</i>".to_string()
        } else {
            s.attendees.iter()
                .map(|a| {
                    let name = registry.get(&a.user_id)
                        .map(|u| u.alias.as_str())
                        .unwrap_or("Unknown");

                    if a.cancelled {
                        format!("<s>{}</s>", name)
                    } else {
                        name.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let count = s.attendees.iter().filter(|a| !a.cancelled).count();
        format!("<b>{} {}</b> @ {} ({} 👥) - <i>{}</i>\n{}\n", s.day, s.activity, s.location, count, s.time, attendees)
    }).collect::<Vec<_>>().join("\n");

    format!("{}{}", header, body)
}

fn generate_log_report(week: &WeekData) -> String {
    let header = format!("📅 <b>Training Log {} to {}</b>\n\n", week.start_date, week.end_date);

    let body = week.sessions.iter().map(|s| {
        let count = s.attendees.iter().filter(|a| !a.cancelled).count();
        format!("<b>{} {}</b> @ {} ({} 👥)\n", s.day, s.activity, s.location, count)
    }).collect::<Vec<_>>().join("\n");

    format!("{}{}", header, body)
}
