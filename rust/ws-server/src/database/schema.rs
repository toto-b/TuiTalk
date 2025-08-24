diesel::table! {
    users (id) {
        id -> Int4,
        room_id -> Int4,
        uuid -> Uuid,
    }
}
