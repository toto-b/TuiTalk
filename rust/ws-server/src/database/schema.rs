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
        time -> BigInt,
        message -> Text,
        username -> Text,
        room_id -> Int4,
        uuid -> Uuid,
        protocol_type -> SmallInt,
    }
}
