![Clipper](icon.png)

# clipper

clip-bot that allows you to clip the last 30 seconds of audio in a voicechat. It automatically joins voicechats. Works on multiple servers. Use server-deathen to disable the bot.

## setup

1. Go to https://discord.com/developers/applications and create an application
   - under "Bot", enable the **Message Content Intent** (needed for the `~info` command)
2. add to server (permissions: View Channels, Send Messages, Attach Files, Connect, Speak)
3. create "#clips" text channel
4. copy server id
5. run the bot with the configuration in the environment (read at runtime, no rebuild needed):
   ```sh
   TOKEN=DISCORD_BOT_TOKEN PORT=SERVER_PORT cargo run --release
   ```
   (if your on linux you might need to install libopus-dev; with nix, `nix develop` provides all dependencies and loads a `.env` file containing these variables)
6. go to http://localhost:{PORT}/clip/{server id}

Clips are written to `output/<server id>/` relative to the working directory.

## custom clip-duration

optionally set `DURATION=MS` in the environment to adjust the clip-size. Replace MS with your duration in milliseconds.

## trigger words

The bot can listen for words or sentences and create a clip when someone says one. It plays a short sound in the voice channel and posts the clip to #clips, mentioning who said it.

1. download the speech recognition library and a model (runs on the CPU, no GPU needed):
   ```sh
   scripts/fetch-moonshine.sh de small   # language: de, en, es, ja, ...; size: tiny, small (medium for en)
   ```
2. put the triggers in `triggers.txt`, one per line. The file is reloaded automatically while the bot runs. Matching ignores case and punctuation ("ok" and "okay" are the same), a phrase may contain a short pause, and in phrases of three or more words one word may be misheard as long as it starts with a similar sound, since names like "Garmin" are often recognized as "Kamin".

Speech recognition uses the [Moonshine](https://github.com/moonshine-ai/moonshine) streaming models, one stream per speaker. The small model has about 7.5% word error rate in german and 7.8% in english, and needs roughly 15-20% of one CPU core per person talking. The tiny model is about 3x cheaper but less accurate (12% word error rate).

Optional environment variables:

| variable | default | |
|---|---|---|
| `TRIGGERS_FILE` | `triggers.txt` | file with the trigger phrases |
| `MOONSHINE_MODEL` | `models/small-streaming-de` | model directory from the fetch script |
| `MOONSHINE_LIB` | `vendor/moonshine-voice/lib/libmoonshine.so` | path to the library |
| `TRIGGER_SOUND` | built-in chime | wav/mp3/ogg file to play instead |
| `TRIGGER_DEBUG` | off | set to `1` to log every sentence the model hears |
| `TRIGGER_KEYTERMS` | off | set to `1` to bias the model towards the trigger phrases (moonshine v0.1.5 sometimes fails to decode with this enabled) |

If the library or model is missing the bot runs without trigger words.

On NixOS, use `nix develop`, which provides the `libstdc++` the prebuilt library needs.
