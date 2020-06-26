create table chats (
    id serial primary key,
    chat_id bigint unique,
    channel_name varchar(100) unique,
    constraint id_name_not_null check (
        not (chat_id is null and channel_name is null)
    )
);