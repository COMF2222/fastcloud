use crate::api::ApiClient;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::OnceLock;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action { Activate, Inbox, Contacts, Open, Messages, Send, Read, Archive, Block, Report, Reports }

fn id(input: &Value, field: &str) -> Result<u64, String> {
    input[field].as_u64().filter(|id| *id > 0 && *id < (1u64 << 53)).ok_or_else(|| "invalid".into())
}

fn request(action: Action, input: &Value) -> Result<(reqwest::Method, String, Option<Value>), String> {
    use reqwest::Method;
    let get = |path: &str| Ok((Method::GET, path.to_owned(), None));
    let post = |path: String, body: Value| Ok((Method::POST, path, Some(body)));
    match action {
        Action::Activate => post("/v1/chat/activate".into(),json!({})),
        Action::Inbox => get("/v1/chat/inbox"),
        Action::Contacts => get("/v1/chat/contacts"),
        Action::Reports => get("/v1/admin/chat/reports"),
        Action::Open => post("/v1/chat/threads".into(),json!({"peerId":id(input,"peerId")?})),
        Action::Block => post("/v1/chat/block".into(),json!({"peerId":id(input,"peerId")?,"blocked":input["blocked"].as_bool().ok_or("invalid")?})),
        Action::Messages => {
            let thread = id(input,"threadId")?;
            let before = input["before"].as_u64().unwrap_or(0);
            let after = input["after"].as_u64().unwrap_or(0);
            if before > 0 && after > 0 { return Err("invalid".into()); }
            get(&format!("/v1/chat/threads/{thread}/messages?before={before}&after={after}"))
        },
        Action::Send => {
            let thread = id(input,"threadId")?;
            let text = input["text"].as_str().filter(|text| text.chars().count() <= 4000).ok_or("invalid")?;
            let nonce = input["nonce"].as_str().filter(|nonce| nonce.len()==32 && nonce.bytes().all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch))).ok_or("invalid")?;
            let attachment = if input["attachment"].is_null() { Value::Null } else {
                let raw=&input["attachment"];
                let kind=raw["kind"].as_str().filter(|kind| matches!(*kind,"track"|"playlist")).ok_or("invalid")?;
                json!({"kind":kind,"id":id(raw,"id")?})
            };
            post(format!("/v1/chat/threads/{thread}/messages"),json!({"text":text,"nonce":nonce,"attachment":attachment}))
        },
        Action::Read => post(format!("/v1/chat/threads/{}/read",id(input,"threadId")?),json!({"through":id(input,"through")?})),
        Action::Archive => post(format!("/v1/chat/threads/{}/archive",id(input,"threadId")?),json!({})),
        Action::Report => {
            let reason=input["reason"].as_str().filter(|reason| matches!(*reason,"spam"|"harassment")).ok_or("invalid")?;
            post(format!("/v1/chat/threads/{}/report",id(input,"threadId")?),json!({"reason":reason}))
        },
    }
}

pub async fn call(client: &ApiClient, action: Action, input: Value) -> Result<Value, String> {
    let (method,path,body)=request(action,&input)?;
    let (server,token)=client.relay_credentials().await.map_err(|_| "network")?.ok_or("sign_in")?;
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    let mut request=HTTP.get_or_init(reqwest::Client::new).request(method,format!("{server}{path}"))
        .timeout(std::time::Duration::from_secs(20)).header("Authorization",format!("OAuth {token}"));
    if let Some(body)=body { request=request.json(&body); }
    let mut response=request.send().await.map_err(|_| "network")?;
    let status=response.status();
    let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|_| "network")? {
        if bytes.len()+chunk.len()>2*1024*1024 { return Err("invalid_response".into()); }
        bytes.extend_from_slice(&chunk);
    }
    let value:Value=serde_json::from_slice(&bytes).map_err(|_| "invalid_response")?;
    if !status.is_success() {
        return Err(value["code"].as_str().unwrap_or(if status.as_u16()==401 {"sign_in"} else if status.as_u16()==400 {"invalid"} else if status.as_u16()==404 {"upgrade_backend"} else {"network"}).to_owned());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_routes_are_fixed_and_never_accept_sender_or_external_urls() {
        let (_,path,body)=request(Action::Send,&json!({"threadId":42,"text":"Hello","nonce":"a".repeat(32),"senderId":999,"attachment":{"kind":"track","id":7,"url":"http://private/token"}})).unwrap();
        assert_eq!(path,"/v1/chat/threads/42/messages");
        assert_eq!(body.unwrap(),json!({"text":"Hello","nonce":"a".repeat(32),"attachment":{"kind":"track","id":7}}));
        assert!(request(Action::Open,&json!({"peerId":"../admin"})).is_err());
        assert!(request(Action::Messages,&json!({"threadId":1,"before":2,"after":3})).is_err());
    }
}
