#[flutter_rust_bridge::frb(sync)] // Synchronous mode for simplicity of the demo
pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}

// Temporal (#33): función nueva sin regenerar el puente; la CI debe fallar.
#[flutter_rust_bridge::frb(sync)]
pub fn ping() -> String {
    "pong".to_owned()
}
