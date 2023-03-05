use anyhow::Result;
use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::{
        events::room::{
            member::StrippedRoomMemberEvent,
            message::{
                MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
                SyncRoomMessageEvent,
            },
        },
        UserId,
    },
    Client,
};
use std::{env, time::Duration};
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let matrix_username = env::var("MATRIX_USERNAME").expect("MATRIX_USERNAME must be set");
    let matrix_password = env::var("MATRIX_PASSWORD").expect("MATRIX_PASSWORD must be set");
    let matrix_user_id = UserId::parse(matrix_username)?;

    let client = Client::builder()
        .server_name(matrix_user_id.server_name())
        .respect_login_well_known(true)
        .handle_refresh_tokens()
        .build()
        .await?;

    client
        .login_username(&matrix_user_id, &matrix_password)
        .initial_device_display_name("ChatGPT")
        .send()
        .await?;

    client.add_event_handler(on_stripped_state_member);

    // An initial sync to set up state and so our bot doesn't respond to old messages.
    let sync_token = client.sync_once(SyncSettings::default()).await?.next_batch;

    client.add_event_handler(on_room_message);

    // Since we called `sync_once` before we entered our sync loop, we must pass that sync token to `sync`.
    let settings = SyncSettings::default().token(sync_token);

    // Syncing is important to synchronize the client state with the server.
    // This method will never return.
    client.sync(settings).await?;

    Ok(())
}

async fn on_room_message(event: SyncRoomMessageEvent, room: Room, client: Client) {
    info!("Received event {:?} in room {:?}", event, room);

    if event.sender() == client.user_id().unwrap() {
        // Skip messages sent by the bot.
        return;
    }

    let Some(event) = event.as_original() else { return };
    // We only want to log text messages in joined rooms.
    let Room::Joined(room) = room else { return };
    let MessageType::Text(ref text_content) = event.content.msgtype else { return };

    info!("Received message {}", text_content.body);

    let response_content = RoomMessageEventContent::text_plain("test response");
    room.send(response_content, None).await;
}

async fn on_stripped_state_member(
    room_member: StrippedRoomMemberEvent,
    client: Client,
    room: Room,
) {
    if room_member.state_key != client.user_id().unwrap() {
        // the invite we've seen isn't for us, but for someone else. ignore
        return;
    }

    if let Room::Invited(room) = room {
        tokio::spawn(async move {
            info!("Autojoining room {}", room.room_id());
            let mut delay = 2;

            while let Err(err) = room.accept_invitation().await {
                // retry autojoin due to synapse sending invites, before the
                // invited user can join for more information see
                // https://github.com/matrix-org/synapse/issues/4345
                error!(
                    "Failed to join room {} ({err:?}), retrying in {delay}s",
                    room.room_id()
                );

                tokio::time::sleep(Duration::from_secs(delay)).await;
                delay *= 2;

                if delay > 3600 {
                    error!("Can't join room {} ({err:?})", room.room_id());
                    break;
                }
            }
            info!("Successfully joined room {}", room.room_id());
        });
    }
}
