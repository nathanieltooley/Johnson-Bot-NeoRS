create table if not exists users (
    id integer primary key not null,
    name text not null,
    vbucks integer not null default 0,
    exp integer not null default 0
);

create table if not exists server_config (
    id integer primary key not null,
    welcome_role_id integer
);
