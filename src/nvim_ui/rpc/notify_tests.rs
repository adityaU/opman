use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct Recorder(Mutex<Vec<(String, Vec<u8>)>>);

impl NotificationSink for Recorder {
    fn notify(&self, method: &str, params: &[u8]) {
        self.0
            .lock()
            .unwrap()
            .push((method.to_owned(), params.to_vec()));
    }
}

#[test]
fn sink_receives_borrowed_method_and_params() {
    let sink = Recorder::default();
    sink.notify("redraw", &[0x91, 0x01]);
    assert_eq!(
        sink.0.lock().unwrap().as_slice(),
        &[(String::from("redraw"), vec![0x91, 0x01])]
    );
}
