# Deck widget protocol

`deck-widget.xml` is an unmodified copy of
[BraiinsForge/bmc-main](https://github.com/BraiinsForge/bmc-main/blob/e0a5c20ceac406ef22f361671516ce5e83e00de8/bmc-widget-protocol/protocol/deck-widget.xml)
at `e0a5c20ceac406ef22f361671516ce5e83e00de8`, verified against GitHub master.
The protocol XML is MIT-licensed; its copyright notice is included in the file.

Generate client bindings directly with `wayland-scanner`. The upstream protocol
crate also includes BMC-specific application/settings types and server bindings;
RetroDeck uses only this wire contract and its existing Wayland client library.

The configured widget UUID comes from `BMC_WIDGET_KEY`. The dashboard and every
widget-presented child use that same instance key. Configuration payload FDs are
closed without reading them because RetroDeck declares no params or credentials.
Wait for `configure_done` before committing the first frame.
