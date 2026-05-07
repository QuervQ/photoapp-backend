# PhotoApp Frontend Spec (SwiftUI, iOS)

## 1. Overview

- Target: iOS native app using SwiftUI.
- Backend: REST + WebSocket.
- Auth: email/password, local JWT + refresh token.
- Storage: rustfs (S3 compatible) with presigned URLs.

## 2. Environments

- Base API URL: `http(s)://<host>:<port>`
- WebSocket URL: `ws(s)://<host>:<port>/ws`

## 3. Auth Model

### Tokens

- `access_token`: JWT, expires in ~7 days.
- `refresh_token`: opaque token, expires in ~30 days.

### Auth Flow

1. Signup or login.
2. Store tokens securely (Keychain).
3. Use `Authorization: Bearer <access_token>` for all protected endpoints.
4. When 401, call refresh to get new tokens.

### Endpoints

#### POST /auth/signup

Request

```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

Response (201)

```json
{
  "access_token": "...",
  "refresh_token": "...",
  "user_id": "uuid",
  "email": "user@example.com"
}
```

#### POST /auth/login

Same request/response as signup.

#### POST /auth/refresh

Request

```json
{
  "refresh_token": "..."
}
```

Response (200) same as signup.

#### GET /me

Response

```json
{
  "user_id": "uuid",
  "email": "user@example.com"
}
```

## 4. Rooms

#### POST /rooms

Request

```json
{ "name": "my room" }
```

Response (201)

```json
{
  "id": "uuid",
  "name": "my room",
  "created_by": "uuid",
  "created_at": "2026-05-07T00:00:00Z"
}
```

#### GET /rooms

Response (200)

```json
[
  {
    "id": "uuid",
    "name": "my room",
    "created_by": "uuid",
    "created_at": "2026-05-07T00:00:00Z"
  }
]
```

#### POST /rooms/{room_id}/invite

Response

```json
{
  "invite_code": "12chars",
  "expires_at": "2026-05-08T00:00:00Z"
}
```

#### POST /rooms/join

Request

```json
{ "invite_code": "12chars" }
```

Response

```json
{ "room_id": "uuid" }
```

## 5. Assets (S3 presigned)

#### POST /assets/upload-url

Request

```json
{
  "kind": "image" | "worldmap",
  "content_type": "image/jpeg",
  "byte_size": 123456
}
```

Response

```json
{
  "asset_id": "uuid",
  "path": "<user_id>/<kind>/<asset_id>",
  "upload_url": "https://...presigned..."
}
```

Client behavior

- Use HTTP `PUT` to `upload_url`.
- Set `Content-Type` to the same value as `content_type` above.
- The upload URL is time-limited.

#### GET /assets/{asset_id}/download-url

Response

```json
{ "download_url": "https://...presigned..." }
```

## 6. Placements

#### POST /rooms/{room_id}/placements

Request

```json
{
  "image_asset_id": "uuid",
  "transform": [16 floats],
  "width_m": 1.0,
  "height_m": 1.0
}
```

Response (201)

```json
{
  "id": "uuid",
  "room_id": "uuid",
  "image_asset_id": "uuid",
  "transform": [16 floats],
  "width_m": 1.0,
  "height_m": 1.0,
  "created_by": "uuid",
  "created_at": "2026-05-07T00:00:00Z"
}
```

#### GET /rooms/{room_id}/placements

Response (200)

```json
[
  {
    "id": "uuid",
    "room_id": "uuid",
    "image_asset_id": "uuid",
    "transform": [16 floats],
    "width_m": 1.0,
    "height_m": 1.0,
    "created_by": "uuid",
    "created_at": "2026-05-07T00:00:00Z",
    "image_download_url": "https://...presigned..."
  }
]
```

## 7. Worldmap

#### POST /rooms/{room_id}/worldmap

Request

```json
{ "asset_id": "uuid" }
```

Response (201)

```json
{
  "version": 1,
  "asset_id": "uuid",
  "download_url": "https://...presigned...",
  "created_at": "2026-05-07T00:00:00Z"
}
```

#### GET /rooms/{room_id}/worldmap

Response (200) same shape as above.

## 8. WebSocket

### Connect

- URL: `/ws`
- Auth: `Authorization: Bearer <access_token>`
- Optional query: `?room_id=<uuid>&token=<access_token>`

### Subscribe message

```json
{ "type": "subscribe", "room_id": "uuid" }
```

### Events

#### placement_created

```json
{
  "type": "placement_created",
  "room_id": "uuid",
  "placement": {
    "id": "uuid",
    "room_id": "uuid",
    "image_asset_id": "uuid",
    "transform": [16 floats],
    "width_m": 1.0,
    "height_m": 1.0,
    "created_by": "uuid",
    "created_at": "2026-05-07T00:00:00Z",
    "image_download_url": "https://...presigned..."
  }
}
```

#### worldmap_updated

```json
{
  "type": "worldmap_updated",
  "room_id": "uuid",
  "version": 2
}
```

#### error

```json
{ "type": "error", "message": "..." }
```

## 9. Error Handling

- Errors are plain text responses with HTTP status codes.
- Treat `401` as token expiration and retry after refresh.
- Treat `403` as access-denied (not a room member).

## 10. SwiftUI Implementation Notes

- Use `URLSession` for REST APIs.
- Use `URLSessionWebSocketTask` for WS; send JSON messages as text frames.
- Store tokens in Keychain; keep a lightweight in-memory cache for request headers.
- Use background task or retry loop for WS reconnect on network loss.
- Keep models `Codable` and map JSON keys 1:1.

## 11. Screen Flow (minimal)

1. Auth
   - Sign up / Login
2. Room List
   - Create room
   - Join room (invite code)
3. Room Detail
   - Placements list
   - Worldmap view
4. Capture/Upload
   - Upload image or worldmap
   - Create placement or set worldmap
