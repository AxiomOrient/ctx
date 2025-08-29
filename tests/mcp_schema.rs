use std::borrow::Cow;
use std::path::{Path, PathBuf};

use futures::{Sink, Stream};
use rmcp::model::*;
use rmcp::service::{serve_directly, RoleServer};

// Local transport using futures mpsc for bidirectional JSON-RPC messages
struct TestSink { tx: tokio::sync::mpsc::UnboundedSender<ServerJsonRpcMessage> }

#[derive(Debug)]
struct TestSinkError;
impl std::fmt::Display for TestSinkError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "sink error") } }
impl std::error::Error for TestSinkError {}

impl Sink<ServerJsonRpcMessage> for TestSink {
    type Error = TestSinkError;
    fn poll_ready(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> { std::task::Poll::Ready(Ok(())) }
    fn start_send(self: std::pin::Pin<&mut Self>, item: ServerJsonRpcMessage) -> Result<(), Self::Error> { self.get_mut().tx.send(item).map_err(|_| TestSinkError) }
    fn poll_flush(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> { std::task::Poll::Ready(Ok(())) }
    fn poll_close(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> { std::task::Poll::Ready(Ok(())) }
}
struct TestStream { rx: tokio::sync::mpsc::UnboundedReceiver<ClientJsonRpcMessage> }

impl Stream for TestStream {
    type Item = ClientJsonRpcMessage;
    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let me = self.get_mut();
        std::pin::Pin::new(&mut me.rx).poll_recv(cx)
    }
}
struct Harness {
    req_tx: tokio::sync::mpsc::UnboundedSender<ClientJsonRpcMessage>,
    resp_rx: tokio::sync::mpsc::UnboundedReceiver<ServerJsonRpcMessage>,
    _guard: rmcp::service::RunningService<RoleServer, ctx::drivers::mcp::sdk::CtxServer>,
}
impl Harness {
    fn start() -> Self {
        let (req_tx, req_rx) = tokio::sync::mpsc::unbounded_channel::<ClientJsonRpcMessage>();
        let (resp_tx, resp_rx) = tokio::sync::mpsc::unbounded_channel::<ServerJsonRpcMessage>();

        let sink = TestSink { tx: resp_tx };
        let stream = TestStream { rx: req_rx };

        let server = ctx::drivers::mcp::sdk::CtxServer::new();
        let running = serve_directly::<RoleServer, _, _, _, rmcp::transport::sink_stream::TransportAdapterSinkStream>(
            server,
            (sink, stream),
            None,
        );

        Harness { req_tx, resp_rx, _guard: running }
    }

    async fn rpc(&mut self, id: u32, req: ClientRequest) -> ServerJsonRpcMessage {
        let id = RequestId::Number(id);
        let msg = ClientJsonRpcMessage::request(req, id);
        self.req_tx.send(msg).expect("send request");
        self.resp_rx.recv().await.expect("receive response")
    }
}
fn out_dir() -> PathBuf { PathBuf::from("tests/golden/mcp") }

fn write_json(path: &Path, v: &serde_json::Value) {
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).ok(); }
    let s = serde_json::to_string_pretty(v).expect("serialize json");
    std::fs::write(path, s + "\n").expect("write file");
}
fn project_response(mut v: serde_json::Value) -> serde_json::Value {
    // For read_resource, drop large text body for determinism
    if let Some(obj) = v.as_object_mut() {
        if let Some(result) = obj.get_mut("result").and_then(|r| r.as_object_mut()) {
            if let Some(contents) = result.get_mut("contents").and_then(|c| c.as_array_mut()) {
                for c in contents.iter_mut() {
                    if let Some(content_obj) = c.as_object_mut() {
                        if content_obj.contains_key("text") {
                            content_obj.insert("text".to_string(), serde_json::Value::String("<omitted>".into()));
                        }
                    }
                }
            }
        }
    }
    v
}
#[tokio::test]
async fn golden_mcp_schema_snapshots() {
    let update = std::env::var("UPDATE_GOLDEN").ok().filter(|v| v == "1").is_some();
    let mut h = Harness::start();

    // 1) List tools
    let tools_resp = h.rpc(1, ClientRequest::ListToolsRequest(ListToolsRequest::default())).await;
    let mut tools_json = serde_json::to_value(&tools_resp).unwrap();
    tools_json = project_response(tools_json);
    let tools_path = out_dir().join("tools_list.out.golden.json");
    if update || !tools_path.exists() { write_json(&tools_path, &tools_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&tools_path).unwrap()).unwrap();
        assert_eq!(tools_json, exp, "tools/list schema changed");
    }

    // 2) Call compose_prompt tool
    let compose_args = rmcp::object!({
        "query": "Snapshot test query",
        "tags": ["test", "golden"],
        "max_tokens": 256
    });
    let compose_req = CallToolRequest::new(CallToolRequestParam { name: Cow::Borrowed("compose_prompt"), arguments: Some(compose_args) });
    let compose_resp = h.rpc(2, ClientRequest::CallToolRequest(compose_req)).await;
    let mut compose_json = serde_json::to_value(&compose_resp).unwrap();
    compose_json = project_response(compose_json);
    let compose_path = out_dir().join("tool_compose_prompt.out.golden.json");
    if update || !compose_path.exists() { write_json(&compose_path, &compose_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&compose_path).unwrap()).unwrap();
        assert_eq!(compose_json, exp, "tools/call compose_prompt schema changed");
    }

    // 3) List resources
    let res_list_resp = h.rpc(3, ClientRequest::ListResourcesRequest(ListResourcesRequest::default())).await;
    let mut res_list_json = serde_json::to_value(&res_list_resp).unwrap();
    res_list_json = project_response(res_list_json);
    let res_list_path = out_dir().join("resources_list.out.golden.json");
    if update || !res_list_path.exists() { write_json(&res_list_path, &res_list_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&res_list_path).unwrap()).unwrap();
        assert_eq!(res_list_json, exp, "resources/list schema changed");
    }

    // 4) Read a resource (first item if available)
    if let ServerJsonRpcMessage::Response(r) = &res_list_resp {
        if let ServerResult::ListResourcesResult(list) = &r.result {
            if let Some(first) = list.resources.first() {
                let uri = first.uri.clone();
                let read_req = ReadResourceRequest::new(ReadResourceRequestParam { uri });
                let read_resp = h.rpc(4, ClientRequest::ReadResourceRequest(read_req)).await;
                let mut read_json = serde_json::to_value(&read_resp).unwrap();
                read_json = project_response(read_json);
                let read_path = out_dir().join("resources_read.out.golden.json");
                if update || !read_path.exists() { write_json(&read_path, &read_json); }
                else {
                    let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&read_path).unwrap()).unwrap();
                    assert_eq!(read_json, exp, "resources/read schema changed");
                }
            }
        }
    }

    // 5) List prompts
    let prompts_resp = h.rpc(5, ClientRequest::ListPromptsRequest(ListPromptsRequest::default())).await;
    let mut prompts_json = serde_json::to_value(&prompts_resp).unwrap();
    prompts_json = project_response(prompts_json);
    let prompts_path = out_dir().join("prompts_list.out.golden.json");
    if update || !prompts_path.exists() { write_json(&prompts_path, &prompts_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&prompts_path).unwrap()).unwrap();
        assert_eq!(prompts_json, exp, "prompts/list schema changed");
    }

    // 6) Get prompt
    let getp_req = GetPromptRequest::new(GetPromptRequestParam { name: "compose_builder".to_string(), arguments: Some(rmcp::object!({"task": "Golden"})) });
    let getp_resp = h.rpc(6, ClientRequest::GetPromptRequest(getp_req)).await;
    let mut getp_json = serde_json::to_value(&getp_resp).unwrap();
    getp_json = project_response(getp_json);
    let getp_path = out_dir().join("prompts_get.out.golden.json");
    if update || !getp_path.exists() { write_json(&getp_path, &getp_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&getp_path).unwrap()).unwrap();
        assert_eq!(getp_json, exp, "prompts/get schema changed");
    }

    // Errors: unknown tool
    let bad_tool_req = CallToolRequest::new(CallToolRequestParam { name: Cow::Borrowed("__unknown__"), arguments: None });
    let bad_tool_resp = h.rpc(7, ClientRequest::CallToolRequest(bad_tool_req)).await;
    let bad_tool_json = serde_json::to_value(&bad_tool_resp).unwrap();
    let bad_tool_path = out_dir().join("error_tools_call_unknown.out.golden.json");
    if update || !bad_tool_path.exists() { write_json(&bad_tool_path, &bad_tool_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&bad_tool_path).unwrap()).unwrap();
        assert_eq!(bad_tool_json, exp, "tools/call unknown error schema changed");
    }

    // Errors: read non-existing resource
    let rr = ReadResourceRequest::new(ReadResourceRequestParam { uri: "doc://does/not/exist.md".into() });
    let rr_resp = h.rpc(8, ClientRequest::ReadResourceRequest(rr)).await;
    let rr_json = serde_json::to_value(&rr_resp).unwrap();
    let rr_path = out_dir().join("error_resources_read_not_found.out.golden.json");
    if update || !rr_path.exists() { write_json(&rr_path, &rr_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&rr_path).unwrap()).unwrap();
        assert_eq!(rr_json, exp, "resources/read not found error schema changed");
    }

    // Errors: get unknown prompt
    let bad_prompt_req = GetPromptRequest::new(GetPromptRequestParam { name: "__unknown__".to_string(), arguments: None });
    let bad_prompt_resp = h.rpc(9, ClientRequest::GetPromptRequest(bad_prompt_req)).await;
    let badp_json = serde_json::to_value(&bad_prompt_resp).unwrap();
    let badp_path = out_dir().join("error_prompts_get_unknown.out.golden.json");
    if update || !badp_path.exists() { write_json(&badp_path, &badp_json); }
    else {
        let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&badp_path).unwrap()).unwrap();
        assert_eq!(badp_json, exp, "prompts/get unknown error schema changed");
    }
}
