//! Funciones y tipos expuestos a Dart. Un archivo por área.

pub mod error;
pub mod session;

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
