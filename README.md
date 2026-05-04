# 🔱 Aquathallyon Bot

A lightweight Telegram bot for club attendance taking, focused on simplicity and background reliability.

## 🚀 Features
- **One-Line Session Entry**: No strict formatting. Just type `MON Swim 5pm @ USC`.
- **Bulk Editing**: Use `/trainings` to get the current list, edit it, and `/edit` to save the whole template.
- **Admin Security**: Only the designated administrator can edit sessions or roll the week.
- **Auto-Roll**: Automatically rolls the week and sends the new schedule every Sunday at 9:00 PM.
- **Local Persistence**: Saves state to `state.json` and a human-readable `sessions.txt`.

## 🛠 Commands
### Public (In Group)
- `/show`: View current week's attendance and sign up.
- `/show_next`: View next week's schedule for early planning.

### Admin Only (DM or Group)
- `/trainings`: Get the raw list of sessions to copy for editing.
- `/edit <list>`: Replace the entire schedule template. (One session per line).
- `/new_week`: Manually force a week rollover.
- `/help`: Show available commands.

## ⚙️ Setup
1. Create a `.env` file in the project root:
   ```env
   TELOXIDE_TOKEN=your_bot_token
   CHAT_ID=-100xxxxxxxxxx  # The group chat where auto-updates are sent
   ADMIN_ID=123456789      # Your personal Telegram User ID
   RUST_LOG=info
   ```
2. Build and run:
   ```bash
   cargo build --release
   ./target/release/aquathallyon
   ```

## 📦 Deployment
The bot compiles to a single static binary. 
1. Run `cargo build --release` on your target machine.
2. Ensure `state.json` and `sessions.txt` (optional) are in the same directory as the binary for persistence.
