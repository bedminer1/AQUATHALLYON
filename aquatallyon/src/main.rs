use teloxide::{
    prelude::*,
};

mod types;
mod handlers;
use crate::types::*;
use crate::handlers::*;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting aquathallyon bot...");

    let initial_state = WeeklyAttendance {
        start_date: "2026-02-02".into(),
        end_date: "2026-02-08".into(),
        sessions: vec![
            TrainingSession { id: 1, activity: "Swim".into(), location: "USC Pool".into(), day: "Monday".into(), attendees: vec![] },
            TrainingSession { id: 2, activity: "Run".into(), location: "NUS Track".into(), day: "Tuesday".into(), attendees: vec![] },
            TrainingSession { id: 3, activity: "Swim".into(), location: "USC Pool".into(), day: "Wednesday".into(), attendees: vec![] },
            TrainingSession { id: 4, activity: "Run".into(), location: "NUS Track".into(), day: "Thursday".into(), attendees: vec![] },
            TrainingSession { id: 5, activity: "Swim".into(), location: "USC Pool".into(), day: "Friday".into(), attendees: vec![] },
            TrainingSession { id: 6, activity: "Bricks".into(), location: "Palawan Beach".into(), day: "Saturday".into(), attendees: vec![] },
        ],
    };

    let state = SharedState::new(initial_state);
    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(Update::filter_message().filter_command::<Command>().endpoint(handle_commands))
        .branch(Update::filter_callback_query().endpoint(receive_btn_press));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}