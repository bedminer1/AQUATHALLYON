use teloxide::{
    prelude::*, 
    types::{ InlineKeyboardMarkup },
};

use crate::types::*;

pub async fn send_menu(
    bot: Bot,
    state: SharedState,
    msg: Message
) -> ResponseResult<()> {
    let (text, keyboard) = {
        let weekly_attendance = state.read();
        let t = generate_attendance_report(&weekly_attendance);
        let k = main_menu_keyboard(&weekly_attendance.sessions);
        (t, k) // Return the data we need, dropping the guard here
    };
    
    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html) // Use V2 for better formatting
        .reply_markup(keyboard)
        .await?;

    Ok(())
}
pub async fn receive_btn_press(
    bot: Bot,
    state: SharedState, 
    q: CallbackQuery,
) -> ResponseResult<()> {
    let user_name = q.from.username.clone().unwrap_or_else(|| "unknown".into());
    let user_id = q.from.id.0;

    let (report_text, keyboard) = {
        let mut week = state.write();

        let session_id = q.data.as_deref()
            .and_then(|data| data.strip_prefix("checkin_"))
            .and_then(|id_str| id_str.parse::<u8>().ok());

        if let Some(session) = session_id.and_then(|id| week.get_session_mut(id)) {
            if let Some(pos) = session.attendees.iter().position(|u| u.telegram_id == user_id) {
                session.attendees.remove(pos);
            } else {
                session.attendees.push(User { telegram_id: user_id, alias: user_name });
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
    let header = format!("📅 <b>Week: {} to {}</b>\n\n", state.start_date, state.end_date);
    
    let body = state.sessions.iter().map(|s| {
        let attendees = if s.attendees.is_empty() {
            "<i>No one yet</i>".to_string()
        } else {
            let list = s.attendees.iter()
                .map(|u| format!("@{}", u.alias))
                .collect::<Vec<_>>()
                .join(", ");
            format!("└ {}", list)
        };
        
        format!("<b>{} {}</b> (@ {})\n{}\n", s.day, s.activity, s.location, attendees)
    }).collect::<Vec<_>>().join("\n");

    format!("{}{}", header, body)
}