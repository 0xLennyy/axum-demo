-- Your SQL goes here
CREATE TABLE postgres."users"(
                        "id" SERIAL PRIMARY KEY,
                        "name" TEXT NOT NULL,
                        "hair_color" TEXT
);