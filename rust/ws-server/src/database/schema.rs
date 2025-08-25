diesel::table! {
    users (id) {
        id -> Int4,
        room_id -> Int4,
        uuid -> Uuid,
    }
}

diesel::table! {
    messages (id) {
        id -> Int4,
        time -> Int4,
        message -> Text,
        username -> Text,
        room_id -> Int4,
        uuid -> Uuid,
    }
}
