use super::scheduler::SchedulerTool;
use super::workspace::Workspace;
use crate::loopback::Loopback;
use axum::{
    extract::Json,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use serde_json::{json, Value};
use tokio::sync::mpsc;

struct Fixture {
    loopback: Loopback,
    requests: mpsc::Receiver<(HeaderMap, Value)>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn fixture(status: StatusCode, reply: Value) -> Fixture {
    let (tx, requests) = mpsc::channel(2);
    let app = Router::new().route(
        "/internal/scheduler",
        post(move |headers: HeaderMap, Json(body): Json<Value>| {
            let tx = tx.clone();
            let reply = reply.clone();
            async move {
                tx.send((headers, body))
                    .await
                    .expect("capture scheduler request");
                (status, Json(reply))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock app");
    let address = listener.local_addr().expect("mock app address");
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve mock app");
    });
    Fixture {
        loopback: Loopback {
            url: format!("http://{address}"),
            token: "scheduler-test-token".into(),
            client: reqwest::Client::new(),
        },
        requests,
        task,
    }
}

fn workspace() -> Workspace {
    Workspace::new(std::path::PathBuf::from("/scheduler-test-project"))
}

#[tokio::test]
async fn every_operation_uses_authenticated_app_engine_and_structured_output() {
    let cases = [
        (
            SchedulerTool::List,
            json!({"all_projects": false}),
            json!({"routines": [], "runs": []}),
        ),
        (
            SchedulerTool::Create,
            json!({"name":"Daily", "prompt":"Check", "cron_expr":"0 9 * * *", "timezone":"UTC"}),
            json!({"id":"routine-1", "name":"Daily"}),
        ),
        (
            SchedulerTool::Update,
            json!({"routine_id":"routine-1", "enabled":false}),
            json!({"id":"routine-1", "name":"Daily", "enabled":false}),
        ),
        (
            SchedulerTool::Delete,
            json!({"routine_id":"routine-1"}),
            json!({"deleted":true, "routine_id":"routine-1"}),
        ),
        (
            SchedulerTool::Run,
            json!({"routine_id":"routine-1"}),
            json!({"id":"run-1", "routine_id":"routine-1", "status":"completed"}),
        ),
    ];
    for (tool, arguments, reply) in cases {
        let mut fixture = fixture(StatusCode::OK, reply.clone()).await;
        let output = tool
            .call_with_loopback(
                &workspace(),
                arguments.clone(),
                Some(fixture.loopback.clone()),
            )
            .await
            .expect("scheduler tool succeeds");
        assert_eq!(output.data, reply);
        assert!(!output.is_error);
        assert!(output.text.contains(tool.name()));
        let (headers, body) = fixture.requests.recv().await.expect("captured request");
        assert_eq!(headers["x-internal-token"], "scheduler-test-token");
        assert_eq!(body["project_dir"], "/scheduler-test-project");
        assert_eq!(body["args"], arguments);
        assert_eq!(
            body["operation"],
            tool.name().trim_start_matches("scheduler_")
        );
    }
}

#[tokio::test]
async fn missing_app_and_invalid_arguments_are_clear_errors() {
    let missing = SchedulerTool::List
        .call_with_loopback(&workspace(), json!({}), None)
        .await
        .err()
        .expect("missing app fails");
    assert!(missing.message().contains("start opman"));
    let invalid = SchedulerTool::Create
        .call_with_loopback(&workspace(), json!([]), None)
        .await
        .err()
        .expect("array arguments fail");
    assert!(invalid.message().contains("expected an object"));
}

#[tokio::test]
async fn authentication_and_engine_errors_keep_exact_reason() {
    for (status, reason) in [
        (StatusCode::UNAUTHORIZED, "invalid internal token"),
        (
            StatusCode::BAD_REQUEST,
            "Invalid cron expression: expected 5 fields",
        ),
        (
            StatusCode::NOT_FOUND,
            "routine was not found in this project",
        ),
    ] {
        let fixture = fixture(status, json!({"error": reason})).await;
        let error = SchedulerTool::Create
            .call_with_loopback(&workspace(), json!({}), Some(fixture.loopback.clone()))
            .await
            .err()
            .expect("server error propagates");
        assert!(error.message().contains(reason));
        assert!(error.message().contains(&status.as_u16().to_string()));
    }
}

#[tokio::test]
async fn invalid_json_shape_is_rejected() {
    let fixture = fixture(StatusCode::OK, json!([])).await;
    let error = SchedulerTool::List
        .call_with_loopback(&workspace(), Value::Null, Some(fixture.loopback.clone()))
        .await
        .err()
        .expect("array reply fails");
    assert!(error.message().contains("expected a JSON object"));
}

#[tokio::test]
async fn stale_descriptor_reports_unreachable_app() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve port");
    let address = listener.local_addr().expect("reserved port");
    drop(listener);
    let loopback = Loopback {
        url: format!("http://{address}"),
        token: "test".into(),
        client: reqwest::Client::new(),
    };
    let error = SchedulerTool::List
        .call_with_loopback(&workspace(), json!({}), Some(loopback))
        .await
        .err()
        .expect("offline app fails");
    assert!(error.message().contains("check that opman is running"));
}

#[test]
fn catalog_explains_side_effects_and_exposes_object_schemas() {
    for tool in SchedulerTool::ALL {
        let definition = tool.definition();
        assert_eq!(definition["name"], tool.name());
        assert_eq!(definition["inputSchema"]["type"], "object");
        assert_eq!(definition["outputSchema"]["type"], "object");
        assert_eq!(
            definition["annotations"]["readOnlyHint"],
            tool == SchedulerTool::List
        );
        assert_eq!(
            definition["annotations"]["destructiveHint"],
            matches!(tool, SchedulerTool::Update | SchedulerTool::Delete)
        );
    }
    let create = SchedulerTool::Create.definition();
    assert_eq!(
        create["inputSchema"]["properties"]["cron_expr"]["type"],
        "string"
    );
    assert_eq!(create["inputSchema"]["additionalProperties"], false);
}
