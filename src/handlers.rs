use teloxide::{
    prelude::*, 
    types::{ InlineKeyboardMarkup },
    utils::command::BotCommands,
};
use chrono::{Duration, Local, Datelike};

use crate::types::*;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Aquathallyon Commands:")]
pub enum Command {
    #[command(description = "More information about commands")]
    Help,
    #[command(description = "start the attendance tracking for the new week.")]
    NewWeek,
    #[command(description = "save current state to db")]
    Save,
}

pub async fn handle_commands(
    bot: Bot,
    state: AppState,
    msg: Message,
    cmd: Command,
) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
        }
        Command::NewWeek => {
            let (report, kb) = {
                let mut weekly_attendance = state.sync_state.write();

                let now = Local::now().date_naive();
                let days_to_next_monday = (7 - now.weekday().num_days_from_monday()) % 7;
                let next_monday = now + Duration::days(days_to_next_monday as i64);
                let next_sunday = next_monday + Duration::days(6);

                weekly_attendance.start_date = next_monday.format("%d/%m").to_string();
                weekly_attendance.end_date = next_sunday.format("%d/%m").to_string();

                // TODO: save state to DB before wiping state

                for session in &mut weekly_attendance.sessions {
                    session.attendees.clear();
                }

                (generate_attendance_report(&weekly_attendance), main_menu_keyboard(&weekly_attendance.sessions))
            };

            bot.send_message(msg.chat.id, format!("{}", report))
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(kb)
                .await?;
        }
        Command::Save => {
            let week_snapshot = state.sync_state.read().clone();

            // 2. Clear current attendance in DB
            state.db.execute("DELETE FROM attendance", ()).await.unwrap();

            // 3. Batch insert the new state
            for session in week_snapshot.sessions {
                for user in session.attendees {
                    state.db.execute(
                        "INSERT INTO attendance (session_id, user_id, user_alias) VALUES (?, ?, ?)",
                        libsql::params![session.id, user.telegram_id, user.alias],
                    ).await.unwrap();
                }
            }

            bot.send_message(msg.chat.id, "✅ Attendance successfully synced to Turso!").await?;
        }
    }

    Ok(())
}

pub async fn receive_btn_press(
    bot: Bot,
    state: AppState, 
    q: CallbackQuery,
) -> ResponseResult<()> {
    let _user_name = q.from.username.clone().unwrap_or_else(|| "unknown".into());
    let display_name = q.from.full_name();
    let user_id = q.from.id.0;

    let (report_text, keyboard) = {
        let mut week = state.sync_state.write();

        let session_id = q.data.as_deref()
            .and_then(|data| data.strip_prefix("checkin_"))
            .and_then(|id_str| id_str.parse::<u8>().ok());

        if let Some(session) = session_id.and_then(|id| week.get_session_mut(id)) {
            if let Some(pos) = session.attendees.iter().position(|u| u.telegram_id == user_id) {
                session.attendees.remove(pos);
            } else {
                session.attendees.push(User { telegram_id: user_id, alias: display_name });
            }
        }
        
        let text = generate_attendance_report(&week);
        let kb = main_menu_keyboard(&week.sessions);
        (text, kb) 
    }; // RwLockWriteGuard drop

    if let Some(teloxide::types::MaybeInaccessibleMessage::Regular(msg)) = q.message {
        bot.edit_message_text(msg.chat.id, msg.id, report_text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(keyboard)
            .await?;
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}

fn main_menu_keyboard(trainings: &[TrainingSession]) -> InlineKeyboardMarkup {
    let rows = trainings
        .iter()
        .map(|s| vec![s.make_button()])
        .collect::<Vec<_>>();

    InlineKeyboardMarkup::new(rows)
}

fn generate_attendance_report(state: &WeeklyAttendance) -> String {
    let header = format!("📅 <b>Training Attendance {} to {}</b>\n\n", state.start_date, state.end_date);
    
    let body = state.sessions.iter().map(|s| {
        let attendees = if s.attendees.is_empty() {
            "<i>No one yet</i>".to_string()
        } else {
            let list = s.attendees.iter()
                .map(|u| format!("{}", u.alias))
                .collect::<Vec<_>>()
                .join("\n");
            format!("{}", list)
        };
        
        format!("<b>{} {}</b> @ {} ({}👥)\n{}\n", s.day, s.activity, s.location, s.attendees.len(), attendees)
    }).collect::<Vec<_>>().join("\n");

    format!("{}{}", header, body)
}