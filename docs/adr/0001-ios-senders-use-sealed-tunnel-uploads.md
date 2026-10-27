# iOS senders upload sealed snapshots through a public tunnel

A Sender can be on any network, so the Desktop app exposes a write-only receive endpoint through the tunnel providers it already has (Tailscale Funnel by default, cloudflared or ngrok as options). Each snapshot is sealed with the Desktop app's public key from pairing, because cloudflared and ngrok end TLS at their edge and could read screenshots of unreleased Ondera UI.

The seal uses HPKE in PSK mode with the sender secret as the PSK and the sender id as the PSK id, so an upload request carries only the sender id, and a tunnel operator who sees the request cannot forge an upload. The secret travels once, in the pairing QR code.

## Considered options

- Tailscale tailnet only: private, but needs Tailscale on the phone and allowed on the work Mac.
- Hosted relay mailbox: no inbound port, but a new service to run and secure.
- TLS only: less code, but the tunnel provider sees plain screenshots.
