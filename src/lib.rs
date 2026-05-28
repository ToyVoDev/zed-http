// Zed expects to find a [package] with a cdylib at the extension root, so
// this crate exists as a thin shim: it pulls in the real extension logic
// from the `zed-http-extension` workspace member and wires it into Zed's
// wasm entry point via the `register_extension!` macro.

zed_extension_api::register_extension!(zed_http_extension::HttpExtension);
