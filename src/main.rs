use anyhow::Result;
use async_openai::{
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequest, Role},
    Chat, Client as OpenAIClient,
};
use lazy_static::lazy_static;
use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::{
        events::room::{
            member::StrippedRoomMemberEvent,
            message::{MessageType, RoomMessageEventContent, SyncRoomMessageEvent},
        },
        UserId,
    },
    Client as MatrixClient,
};
use std::{env, time::Duration};
use tracing::{debug, error, info};

lazy_static! {
    static ref OPENAI_CLIENT: OpenAIClient = {
        let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
        let openai_client = OpenAIClient::new().with_api_key(openai_api_key);
        openai_client
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let matrix_username = env::var("MATRIX_USERNAME").expect("MATRIX_USERNAME must be set");
    let matrix_password = env::var("MATRIX_PASSWORD").expect("MATRIX_PASSWORD must be set");
    let matrix_user_id = UserId::parse(matrix_username)?;

    let matrix_client = MatrixClient::builder()
        .server_name(matrix_user_id.server_name())
        .respect_login_well_known(true)
        .handle_refresh_tokens()
        .build()
        .await?;

    matrix_client
        .login_username(&matrix_user_id, &matrix_password)
        .initial_device_display_name("ChatGPT")
        .send()
        .await?;

    matrix_client.add_event_handler(on_stripped_state_member);

    // An initial sync to set up state and so our bot doesn't respond to old messages.
    let sync_token = matrix_client
        .sync_once(SyncSettings::default())
        .await?
        .next_batch;

    matrix_client.add_event_handler(on_room_message);

    // Since we called `sync_once` before we entered our sync loop, we must pass that sync token to `sync`.
    let settings = SyncSettings::default().token(sync_token);

    // Syncing is important to synchronize the client state with the server.
    // This method will never return.
    matrix_client.sync(settings).await?;

    Ok(())
}

async fn on_room_message(event: SyncRoomMessageEvent, room: Room, client: MatrixClient) {
    debug!("Received event {:?} in room {:?}", event, room);

    if event.sender() == client.user_id().unwrap() {
        // Skip messages sent by the bot.
        return;
    }

    let Some(event) = event.as_original() else { return };
    // We only want to log text messages in joined rooms.
    let Room::Joined(room) = room else { return };
    let MessageType::Text(ref text_content) = event.content.msgtype else { return };

    debug!("Received message: {}", text_content.body);

    let chatgpt_request = CreateChatCompletionRequest {
        model: "gpt-3.5-turbo".into(),
        messages: vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: text_content.body.to_string(),
            name: None, // TODO: get user name
        }],
        temperature: None,
        top_p: None,
        n: Some(1),
        stream: Some(false),
        stop: None,
        max_tokens: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: Some("matrix-chatgpt".into()),
    };
    let Ok(chatgpt_response) = Chat::new(&OPENAI_CLIENT).create(chatgpt_request).await else { return; };

    let response = chatgpt_response.choices[0].message.content.clone();

    debug!("Sending ChatGPT response: {}", response);

    let response_content = RoomMessageEventContent::text_markdown(response);

    room.send(response_content, None)
        .await
        .map_err(|e| {
            error!("Failed to send message: {:?}", e);
        })
        .ok();
}

async fn on_stripped_state_member(
    room_member: StrippedRoomMemberEvent,
    client: MatrixClient,
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
