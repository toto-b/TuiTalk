CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    room_id INTEGER,
    uuid UUID
);

CREATE TABLE messages (
    id SERIAL PRIMARY KEY,
    time INTEGER,
    message TEXT,
    username TEXT,
    room_id INTEGER,
    uuid UUID
);
