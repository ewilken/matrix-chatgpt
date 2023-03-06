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
    static ref AUTHORIZED_USERS: Vec<String> = {
        let Ok(authorized_users_string) = env::var("AUTHORIZED_USERS") else { return vec![]; };
        let authorized_users = authorized_users_string
            .split(',')
            .map(|s| s.to_string())
            .collect();

        authorized_users
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
        .initial_device_display_name("matrix-chatgpt")
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

    if let Some(user_id) = client.user_id() {
        // Skip messages sent by the bot itself.
        if event.sender() == user_id {
            return;
        }

        // If we have an authorized users list, ignore messages from unauthorized users.
        if AUTHORIZED_USERS.len() > 0 && !AUTHORIZED_USERS.contains(&user_id.to_string()) {
            debug!("Ignoring message from unauthorized user {}", user_id);
            return;
        }
    }

    let Some(event) = event.as_original() else { return };

    // We only want to process text messages from rooms the bot joined.
    let Room::Joined(joined_room) = room else { return };
    let MessageType::Text(ref text_content) = event.content.msgtype else { return };

    joined_room
        .read_receipt(&event.event_id)
        .await
        .map_err(|e| {
            error!("Failed to send read receipt: {:?}", e);
        })
        .ok();

    debug!("Received message: {}", text_content.body);

    // let timeline = room.timeline().await;

    joined_room
        .typing_notice(true)
        .await
        .map_err(|e| {
            error!("Failed to send typing notice: {:?}", e);
        })
        .ok();

    let chatgpt_request = CreateChatCompletionRequest {
        model: "gpt-3.5-turbo".into(),
        messages: vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: text_content.body.to_string(),
            name: Some(event.sender.to_string()),
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

    joined_room
        .send(RoomMessageEventContent::text_markdown(response), None)
        .await
        .map_err(|e| {
            error!("Failed to send answer: {:?}", e);
        })
        .ok();
}

async fn on_stripped_state_member(
    room_member: StrippedRoomMemberEvent,
    client: MatrixClient,
    room: Room,
) {
    if room_member.state_key != client.user_id().unwrap() {
        // The invite we've seen isn't for us, but for someone else. Ignore.
        return;
    }
    let Room::Invited(room) = room else {
        return;
    };

    tokio::spawn(async move {
        info!("Autojoining room {}", room.room_id());
        let mut delay = 2;

        while let Err(err) = room.accept_invitation().await {
            // Retry autojoin due to synapse sending invites before the
            // invited user can join. For more information see
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
