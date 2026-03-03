## Plan: Rust単独認証＋AR共有MVP実装

このDRAFTは、現状が最小雛形のみのため、土台構築→認証→Room→Storage署名URL→WorldMap→Placement配信の順で一本道に固定した実装計画です。決定事項は、Storageは署名URL方式、招待コードは24時間TTLの複数利用、WebSocket認証はAuthorizationヘッダ正規＋query互換、DBマイグレーションはsqlx migrateです。これにより「クライアントはSupabase鍵を保持しない」「Rustが認証と署名URL発行を一元管理」「room単位でAR状態を同期」の3条件をMVP段階で満たし、拡張時も破綻しない構成にします。

**Steps**

1. 実行基盤を作成
   - 依存追加とプロジェクト骨格化を実施: [Cargo.toml](Cargo.toml), [src/main.rs](src/main.rs)
   - 追加予定: [src/config.rs](src/config.rs), [src/app_state.rs](src/app_state.rs), [src/error.rs](src/error.rs), [src/routes/mod.rs](src/routes/mod.rs)

2. Docker開発環境を固定
   - postgres + backendのみをcompose化し、ローカル起動を一本化
   - 追加予定: [docker-compose.yml](docker-compose.yml), [Dockerfile](Dockerfile), [.env.example](.env.example)

3. 設定とヘルスチェックを先に完成
   - 必須環境変数を起動時検証し、healthzでDB到達性も確認
   - 反映先: [src/config.rs](src/config.rs), [src/routes/health.rs](src/routes/health.rs), [src/main.rs](src/main.rs)

4. DBマイグレーション基盤を導入
   - sqlx migrateを採用し、MVPスキーマを段階投入
   - 追加予定: [migrations/0001_init.sql](migrations/0001_init.sql), [migrations/0002_worldmap_placements.sql](migrations/0002_worldmap_placements.sql)

5. 認証（Rust完結）を実装
   - signup/login/me、argon2ハッシュ、JWT発行・検証ミドルウェア
   - 追加予定: [src/routes/auth.rs](src/routes/auth.rs), [src/auth/jwt.rs](src/auth/jwt.rs), [src/auth/password.rs](src/auth/password.rs), [src/middleware/auth.rs](src/middleware/auth.rs)

6. Room機能を実装
   - rooms作成、inviteコード発行（24h/複数利用）、join、rooms一覧
   - 追加予定: [src/routes/rooms.rs](src/routes/rooms.rs), [src/services/invite.rs](src/services/invite.rs)

7. Supabase Storage署名URL発行を実装
   - upload-url発行、asset登録、complete、download-url発行をRustで一元化
   - 追加予定: [src/routes/assets.rs](src/routes/assets.rs), [src/integrations/supabase_storage.rs](src/integrations/supabase_storage.rs)

8. WorldMap共有APIを実装
   - roomごと最新版(version管理)登録・取得、取得時にdownload_url付与
   - 追加予定: [src/routes/worldmaps.rs](src/routes/worldmaps.rs)

9. Placement永続化とリアルタイム配信を実装
   - placements作成/取得、room購読WS、placement_created/worldmap_updated配信
   - 追加予定: [src/routes/placements.rs](src/routes/placements.rs), [src/routes/ws.rs](src/routes/ws.rs), [src/ws/hub.rs](src/ws/hub.rs)

10. 最終ハードニングとドキュメント

- CORS/WS許可オリジン、入力バリデーション、サイズ/MIME制限、最小運用手順
- 追加予定: [README.md](README.md)

**Verification**

- ローカル起動: docker compose up で postgres/backendが起動し、GET /healthz がDB接続成功を返す
- 認証: signup→login→me のE2E確認（不正JWT/期限切れJWTも確認）
- Room: create→invite→join→list の一連確認
- Storage: upload-url取得→クライアント直接PUT→complete→download-url取得→GET確認
- AR核: worldmap登録後に購読クライアントへ worldmap_updated、placement作成後に placement_created が同一roomへ配信
- 回帰: sqlx migrate reset→migrate run→主要API再確認

**Decisions**

- Storage方式: 署名URL方式（Rustのみが発行主体）
- 招待コード: 24時間TTL＋複数利用
- WS認証: Authorizationヘッダ正規、query token互換
- Migration: sqlx migrate
- 認証基盤: Supabase Auth不使用、Rust JWTのみ
