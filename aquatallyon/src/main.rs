use teloxide::{
    prelude::*, 
    types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage},
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(send_menu))
        .branch(Update::filter_callback_query().endpoint(receive_callback));

    Dispatcher::builder(bot, handler).enable_ctrlc_handler().build().dispatch().await;
}

fn main_menu_keyboard() -> InlineKeyboardMarkup {
    let help_button = InlineKeyboardButton::callback("Help ℹ️", "help");
    let greet_button = InlineKeyboardButton::callback("Greet Me 👋", "greet");

    // to stack vertically
    InlineKeyboardMarkup::new([
        [help_button],
        [greet_button],
    ])
}

async fn send_menu(bot: Bot, msg: Message) -> ResponseResult<()> {

    bot.send_message(msg.chat.id, "Welcome to AquaTallyon! Choose an option:")
        .reply_markup(main_menu_keyboard())
        .await?;

    Ok(())
}

async fn receive_callback(bot: Bot, q: CallbackQuery) -> ResponseResult<()> {
    let user_name = q.from.username.as_deref().unwrap_or("Friend");
    
    let text = match q.data.as_deref() {
        Some("help") => "This bot tracks attendance for NUS Aquathlon. Click Greet to test!".to_string(),
        Some("greet") => format!("Hello, @{}! Ready for the set?", user_name),
        _ => "Unknown action".to_string(),
    };

    if let Some(MaybeInaccessibleMessage::Regular(msg)) = q.message {
        bot.edit_message_text(msg.chat.id, msg.id, text)
            .reply_markup(main_menu_keyboard())
            .await?;
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}