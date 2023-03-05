# Matrix-ChatGPT

Matrix bridge to the ChatGPT API.

## Usage

    docker build -t ghcr.io/ewilken/matrix-chatgpt .
    docker run \
      -e OPENAI_API_KEY="yourkey" \
      -e MATRIX_USERNAME="@chatgpt:yourhomeserver.tld" \
      -e MATRIX_PASSWORD="yourpassword" \
      ghcr.io/ewilken/matrix-chatgpt
