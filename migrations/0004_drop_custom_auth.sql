-- Supabase Auth に統一するためカスタム認証テーブルを削除
DROP TABLE IF EXISTS refresh_tokens;
DROP TABLE IF EXISTS users;
