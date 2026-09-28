![Clipper](icon.png)

# clipper

clip-bot that allows you to clip the last 30 seconds of audio in a voicechat. It automatically joins voicechats. Works on multiple servers. Use server-deathen to disable the bot.

## setup

1. Go to https://discord.com/developers/applications and create an application
2. add to server
3. create "#clips" text channel
4. copy server id
5. run the bot with the configuration in the environment (read at runtime, no rebuild needed):
   ```sh
   TOKEN=DISCORD_BOT_TOKEN PORT=SERVER_PORT cargo run --release
   ```
   (if your on linux you might need to install libopus-dev)
6. go to http://localhost:{PORT}/clip/{server id}

Clips are written to `output/<server id>/` relative to the working directory.

## custom clip-duration

optionally set `DURATION=MS` in the environment to adjust the clip-size. Replace MS with your duration in milliseconds.
