//! Neovim UI attachment and identity requests.

use anyhow::Result;
use rmpv::Value;

use super::key::UiSize;
use super::rpc::NvimClient;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiOptions {
    pub ext_linegrid: bool,
    pub ext_cmdline: bool,
    pub ext_messages: bool,
    pub ext_popupmenu: bool,
    pub ext_tabline: bool,
    pub rgb: bool,
    pub ext_multigrid: bool,
    pub ext_hlstate: bool,
    pub ext_termcolors: bool,
}

impl Default for UiOptions {
    fn default() -> Self {
        Self {
            ext_linegrid: true,
            ext_cmdline: true,
            ext_messages: true,
            ext_popupmenu: true,
            ext_tabline: true,
            rgb: true,
            ext_multigrid: true,
            ext_hlstate: false,
            ext_termcolors: false,
        }
    }
}

impl UiOptions {
    pub fn as_value(self) -> Value {
        Value::Map(vec![
            (
                Value::from("ext_linegrid"),
                Value::Boolean(self.ext_linegrid),
            ),
            (Value::from("ext_cmdline"), Value::Boolean(self.ext_cmdline)),
            (
                Value::from("ext_messages"),
                Value::Boolean(self.ext_messages),
            ),
            (
                Value::from("ext_popupmenu"),
                Value::Boolean(self.ext_popupmenu),
            ),
            (Value::from("ext_tabline"), Value::Boolean(self.ext_tabline)),
            (Value::from("rgb"), Value::Boolean(self.rgb)),
            (
                Value::from("ext_multigrid"),
                Value::Boolean(self.ext_multigrid),
            ),
            (Value::from("ext_hlstate"), Value::Boolean(self.ext_hlstate)),
            (
                Value::from("ext_termcolors"),
                Value::Boolean(self.ext_termcolors),
            ),
        ])
    }
}

pub async fn set_client_info(client: &NvimClient) -> Result<Value> {
    client
        .request(
            "nvim_set_client_info",
            Value::Array(vec![
                Value::from("opman"),
                version(),
                Value::from("ui"),
                Value::Map(Vec::new()),
                Value::Map(Vec::new()),
            ]),
        )
        .await
}

pub async fn ui_attach(client: &NvimClient, size: UiSize) -> Result<Value> {
    client
        .request(
            "nvim_ui_attach",
            Value::Array(vec![
                Value::from(u64::from(size.cols())),
                Value::from(u64::from(size.rows())),
                UiOptions::default().as_value(),
            ]),
        )
        .await
}

pub async fn ui_detach(client: &NvimClient) -> Result<Value> {
    client
        .request("nvim_ui_detach", Value::Array(Vec::new()))
        .await
}

pub async fn ui_try_resize(client: &NvimClient, size: UiSize) -> Result<Value> {
    client
        .request(
            "nvim_ui_try_resize",
            Value::Array(vec![
                Value::from(u64::from(size.cols())),
                Value::from(u64::from(size.rows())),
            ]),
        )
        .await
}

fn version() -> Value {
    Value::Map(vec![
        (Value::from("major"), Value::from(0u64)),
        (Value::from("minor"), Value::from(1u64)),
        (Value::from("patch"), Value::from(0u64)),
    ])
}

#[cfg(test)]
#[path = "attach_tests.rs"]
mod attach_tests;
