# Velocity Remote Audio Streaming

Velocity can accept a mobile microphone stream over a local network WebSocket.
This prototype is intended for trusted LAN use, such as an Android device and a
Windows 11 computer on the same Wi-Fi network.

The server is compiled in by default with the `remote-audio` Cargo feature. A
build made with `cargo build -p velocity --no-default-features` omits the
listener and hides the remote audio Settings controls. Even when the feature is
compiled in, the listener remains off until the user enables `Accept mobile
microphone streams` in Velocity Settings.

## Endpoint

- Transport: WebSocket over TCP
- Default server port: `54545`
- Default URL: `ws://<windows-host>:54545/v1/audio`
- TLS: not used by the prototype
- One active mobile client is accepted at a time

The Velocity Settings window controls whether the server is enabled and which
port it listens on. If the mobile client leaves the port blank, it should use
the default port `54545`.

## Audio Format

Version 1 uses raw PCM frames:

- Encoding: `linear16`
- Sample format: signed 16-bit little-endian PCM
- Sample rate: `16000` Hz
- Channels: `1`
- WebSocket binary messages: one or more whole PCM samples

The client should send audio as binary WebSocket messages. A message can contain
any practical number of samples, but 20-100 ms chunks are recommended for a
responsive prototype.

## Session Messages

After the WebSocket opens, the client may send a text JSON hello message:

```json
{
  "type": "start",
  "version": 1,
  "encoding": "linear16",
  "sample_rate": 16000,
  "channels": 1
}
```

Velocity responds with a text JSON ready message when it has connected the
stream to Deepgram:

```json
{
  "type": "ready",
  "encoding": "linear16",
  "sample_rate": 16000,
  "channels": 1
}
```

If Velocity cannot accept the stream, it sends a text JSON error message and
closes the WebSocket:

```json
{
  "type": "error",
  "message": "Velocity already has an active remote audio client"
}
```

The client stops streaming by closing the WebSocket.

## Opus Extension Point

Opus can be added to this protocol later, but both sides need to agree on exact
packet framing. The recommended future shape is:

- `encoding`: `opus`
- Sample rate: `48000` Hz
- Channels: `1`
- Frame duration: `20` ms
- Binary messages: one self-contained Opus packet per WebSocket message

PCM is used for version 1 because the Flutter app already has a reliable PCM16
microphone stream path and Velocity can forward that stream directly into its
existing Deepgram streaming pipeline.
