create table if not exists friendships (
    user_from integer not null,
    user_to integer not null,
    relation_type integer not null,
    PRIMARY KEY (user_from, user_to)
);
