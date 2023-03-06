# Matrix-ChatGPT

Matrix bridge to the ChatGPT API.

This is a quick and dirty, probably flawed implementation of a Matrix bot that uses the [ChatGPT API](https://platform.openai.com/docs/guides/chat) to generate responses to messages sent in Matrix rooms. It's based on the official [`matrix-rust-sdk`](https://github.com/matrix-org/matrix-rust-sdk) and the [`async-openai`](https://github.com/64bit/async-openai) wrapper.

## Configuration

The bot can be configured via the following environment variables:

| Variable           | Mandatory | Description                                                                                                                                                                                                                          |
| ------------------ | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `OPENAI_API_KEY`   | true      | OpenAI API key.                                                                                                                                                                                                                      |
| `MATRIX_USERNAME`  | true      | Matrix username of the bot's account.                                                                                                                                                                                                |
| `MATRIX_PASSWORD`  | true      | Matrix password of the bot's account.                                                                                                                                                                                                |
| `AUTHORIZED_USERS` | false     | Comma-separated list of Matrix users that are allowed to use the bot. If set, the bot will only generate answers to messages sent by accounts on the list. If not set, the bot will answer any message on any room it gets added to. |

## Shortcomings

- Due to lack of implementation effort, at the moment the bot is stateless. Matrix client sessions are not persisted nor recovered upon service restart. Meaning that after a service restart, the ChatGPT conversion of every room the bot is in will start over, as messages from sessions previous to its restart can't be decrypted by the bot. In reality, this shouldn't hurt too much, as ChatGPT conversations seem to be of rather short, ephemeral nature.

## Usage

    docker build -t ghcr.io/ewilken/matrix-chatgpt .
    docker run \
      -e OPENAI_API_KEY="yourkey" \
      -e MATRIX_USERNAME="@chatgpt:yourhomeserver.tld" \
      -e MATRIX_PASSWORD="yourpassword" \
      -e AUTHORIZED_USERS="@user1:yourhomeserver.tld,@user2:yourhomeserver.tld" \
      ghcr.io/ewilken/matrix-chatgpt
