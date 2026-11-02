# Deck widget protocol

`deck-widget.xml` is an unmodified copy of
[BraiinsForge/bmc-main](https://github.com/BraiinsForge/bmc-main/blob/b2c945b5f7a8ea3a7d38a8410a1ca64e8b69b9af/bmc-widget-protocol/protocol/deck-widget.xml)
at `b2c945b5f7a8ea3a7d38a8410a1ca64e8b69b9af`, verified against GitHub master
on 2026-11-02.
The protocol XML is MIT-licensed; its copyright notice is included in the file.

Generate client bindings directly with `wayland-scanner`. The upstream protocol
crate also includes BMC-specific application/settings types and server bindings;
RetroDeck uses only this wire contract and its existing Wayland client library.
Negotiate manager/surface version 2, with version 1 compatibility. Version 2 adds
the optional language event; RetroDeck currently uses the protocol's English
fallback for every device language.

The configured widget UUID comes from `BMC_WIDGET_KEY`. The dashboard and every
widget-presented child use that same instance key. Configuration payload FDs are
closed without reading them because RetroDeck declares no params or credentials.
Wait for `configure_done` before committing the first frame.
