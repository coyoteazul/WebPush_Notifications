# web_notif — Web Push Notifications server

A small Rust server that exposes endpoints to deliver Web Push notifications using VAPID keys.
OpenSSL is packed on the server by using "openssl --vendored"
By default the app will look for a file named `conf.json` in the current working directory. If it is not found the server will generate a new VAPID keypair and write a default `conf.json`

Openapi specification is detailed in
`GET /openapi.json`
  
Example: send a notification (minimal example)

Create `payload.json`:

```json
{
  "subscription": {
    "endpoint": "https://fcm.googleapis.com/fcm/send/abc...",
    "keys": { "p256dh": "...", "auth": "..." }
  },
  "payload": {
    "notification": {
      "title": "Hello",
      "body": "This is a test push notification"
    }
  }
}
```
