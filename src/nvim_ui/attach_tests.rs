use super::*;
use rmpv::Value;

#[test]
fn default_options_are_the_single_grid_ui_contract() {
    assert_eq!(
        UiOptions::default().as_value(),
        Value::Map(vec![
            (Value::from("ext_linegrid"), Value::Boolean(true)),
            (Value::from("ext_cmdline"), Value::Boolean(true)),
            (Value::from("ext_messages"), Value::Boolean(true)),
            (Value::from("ext_popupmenu"), Value::Boolean(true)),
            (Value::from("ext_tabline"), Value::Boolean(true)),
            (Value::from("rgb"), Value::Boolean(true)),
            (Value::from("ext_multigrid"), Value::Boolean(true)),
            (Value::from("ext_hlstate"), Value::Boolean(false)),
            (Value::from("ext_termcolors"), Value::Boolean(false)),
        ])
    );
}
