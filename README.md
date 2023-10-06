![Clipper](icon.png)

# clipper

clip-bot that allows you to clip the last 30 seconds of audio in a voicechat. It automatically joins voicechats. Works on multiple servers. Use server-deathen to disable the bot.

## setup

1. create .cargo folder
2. create config.toml inside .cargo
3. write the following configuration the config.toml replacing DISCORD_BOT_TOKEN with your bot token. And replacing SERVER_PORT with a free port. (see config.toml.example)

```toml
[env]
TOKEN = "DISCORD_BOT_TOKEN"
PORT = "SERVER_PORT"
```

4. Go to https://discord.com/developers/applications and create an application
5. add to server
6. create "#clips" text channel
7. copy server id
8. run `cargo run`<br>
   (if your on linux you might need to install libopus-dev)
9. go to http://localhost:{PORT}/clip/{server id}

## custom clip-duration

optionally you can add DURATION="MS" your config.toml to adjust the clip-size. Replace MS with your duration in milliseconds.
