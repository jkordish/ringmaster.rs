pub const MIGRATIONS: &[&str] = &[
    "create table if not exists app_metadata (key text primary key, value text not null);",
    "create table if not exists sync_state (source text primary key, cursor text, updated_at text);",
    "create table if not exists raw_payloads (id integer primary key, source text not null, payload text not null);",
];
