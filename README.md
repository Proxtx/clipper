![Clipper](icon.png)

# clipper

clip-bot that allows you to clip the last 30 seconds of audio in a voicechat. It automatically joins voicechats. Works on multiple servers.

## setup

1. create .cargo folder
2. create config.toml inside .cargo
3. write the following configuration the config.toml replacing DISCORD_BOT_TOKEN with your bot token. And replacing SERVER_PORT with a free port. (see config.toml.example)

```toml
[env]
TOKEN = "DISCORD_BOT_TOKEN"
PORT = "SERVER_PORT"
```

4. add to server
5. create "#clips" text channel
6. copy server id
7. run `cargo run`<br>
   (if your on linux you might need to install libopus-dev)
8. go to http://localhost:{PORT}/clip/{server id}
