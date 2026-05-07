use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use tokio_tungstenite::connect_async;
use uuid::Uuid;

const BASE_URL: &str = "http://localhost:8080";
const WS_URL: &str = "ws://localhost:8080";

#[tokio::test]
#[ignore = "requires running backend at localhost:8080"]
async fn api_flow_signup_room_placement() {
    let client = Client::new();
    let token = signup_and_get_token(&client).await;

    let me = client
        .get(format!("{BASE_URL}/me"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("me request failed");
    assert_eq!(me.status(), 200);

    let room = client
        .post(format!("{BASE_URL}/rooms"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "e2e-room"}))
        .send()
        .await
        .expect("room create request failed");
    assert_eq!(room.status(), 201);

    let room_json: Value = room.json().await.expect("room json parse failed");
    let room_id = room_json
        .get("id")
        .and_then(Value::as_str)
        .expect("room id missing")
        .to_string();

    let placement = client
        .post(format!("{BASE_URL}/rooms/{room_id}/placements"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "image_asset_id": Uuid::new_v4(),
            "transform": [1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0],
            "width_m": 1.0,
            "height_m": 1.0
        }))
        .send()
        .await
        .expect("placement request failed");
    assert_eq!(placement.status(), 201);

    let list = client
        .get(format!("{BASE_URL}/rooms/{room_id}/placements"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("placement list request failed");
    assert_eq!(list.status(), 200);

    let list_json: Value = list.json().await.expect("placements json parse failed");
    assert!(list_json.as_array().is_some_and(|array| !array.is_empty()));
}

#[tokio::test]
#[ignore = "requires running backend at localhost:8080"]
async fn ws_receives_placement_created_event() {
    let client = Client::new();
    let token = signup_and_get_token(&client).await;

    let room = client
        .post(format!("{BASE_URL}/rooms"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "ws-room"}))
        .send()
        .await
        .expect("room create request failed");
    assert_eq!(room.status(), 201);

    let room_json: Value = room.json().await.expect("room json parse failed");
    let room_id = room_json
        .get("id")
        .and_then(Value::as_str)
        .expect("room id missing")
        .to_string();

    let ws_endpoint = format!("{WS_URL}/ws?room_id={room_id}&token={token}");
    let (mut ws_stream, _) = connect_async(ws_endpoint)
        .await
        .expect("websocket connect failed");

    let placement_req = client
        .post(format!("{BASE_URL}/rooms/{room_id}/placements"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "image_asset_id": Uuid::new_v4(),
            "transform": [1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0],
            "width_m": 1.0,
            "height_m": 1.0
        }))
        .send()
        .await
        .expect("placement request failed");
    assert_eq!(placement_req.status(), 201);

    let message = ws_stream
        .next()
        .await
        .expect("expected websocket message")
        .expect("websocket frame error")
        .into_text()
        .expect("text frame expected");

    let event: Value = serde_json::from_str(&message).expect("event json parse failed");
    assert_eq!(event.get("type").and_then(Value::as_str), Some("placement_created"));
    assert_eq!(
        event.get("room_id").and_then(Value::as_str),
        Some(room_id.as_str())
    );
}

async fn signup_and_get_token(client: &Client) -> String {
    let email = format!("e2e-{}@example.com", Uuid::new_v4());
    let signup = client
        .post(format!("{BASE_URL}/auth/signup"))
        .json(&serde_json::json!({
            "email": email,
            "password": "password123"
        }))
        .send()
        .await
        .expect("signup request failed");

    assert_eq!(signup.status(), 201);
    let payload: Value = signup.json().await.expect("signup json parse failed");

    let refresh_token = payload
        .get("refresh_token")
        .and_then(Value::as_str)
        .expect("refresh_token missing")
        .to_string();

    let refreshed = client
        .post(format!("{BASE_URL}/auth/refresh"))
        .json(&serde_json::json!({
            "refresh_token": refresh_token
        }))
        .send()
        .await
        .expect("refresh request failed");

    assert_eq!(refreshed.status(), 200);
    let refreshed_payload: Value = refreshed
        .json()
        .await
        .expect("refresh json parse failed");

    refreshed_payload
        .get("access_token")
        .and_then(Value::as_str)
        .expect("access_token missing")
        .to_string()
}
