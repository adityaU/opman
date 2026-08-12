//! End-to-end checks against a real embedded Neovim process.
use rmpv::Value;

#[path = "live_helpers.rs"]
mod helpers;

#[path = "live_ws_tests.rs"]
mod websocket;

#[path = "live_pool_tests.rs"]
mod pool_tests;

#[path = "canvas_live_tests.rs"]
mod canvas;

#[path = "../nvim_edit/live_tests.rs"]
mod edit_engine;

use helpers::{fixture, have_nvim, lock, start};

#[tokio::test]
#[ignore = "spawns a real Neovim"]
async fn frontend_key_table_round_trips_through_neovim_parser() {
    if !have_nvim() {
        eprintln!("skipping: nvim not installed");
        return;
    }
    let _live_guard = lock().await;
    let project = fixture();
    let (session, _) = start(&project, "keys").await;
    let table: Vec<String> =
        serde_json::from_str(include_str!("../../web-ui/src/nvim/__fixtures__/keys.json"))
            .expect("key fixture");
    for notation in table {
        let escaped = notation.replace('\'', "''");
        let expression =
            format!("keytrans(nvim_replace_termcodes('{escaped}', v:true, v:true, v:true))");
        let value = session
            .client()
            .request("nvim_eval", Value::Array(vec![Value::from(expression)]))
            .await
            .expect("keytrans RPC");
        assert_eq!(
            value.as_str(),
            Some(notation.as_str()),
            "Neovim rejected key notation {notation:?}: {value:?}"
        );
    }
    session.shutdown().await;
}
