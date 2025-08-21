CREATE TABLE "user" (
    id SERIAL PRIMARY KEY,
    room_id INTEGER,
    uuid UUID
);

CREATE TABLE message (
    id SERIAL PRIMARY KEY,
    time INTEGER,
    message TEXT,
    room_id INTEGER,
    user_id INTEGER REFERENCES "user"(id)
);
