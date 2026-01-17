-- Add migration script here
PRAGMA foreign_keys = ON;

CREATE TABLE rarities (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE types (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE sets (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  short_name TEXT NOT NULL UNIQUE,
  release_date TEXT NOT NULL,
  card_count INTEGER NOT NULL
);

CREATE TABLE cards (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  rarity_id INTEGER NOT NULL,
  type_id INTEGER NOT NULL,
  set_id INTEGER NOT NULL,
  FOREIGN KEY (rarity_id) REFERENCES rarities(id),
  FOREIGN KEY (type_id) REFERENCES types(id),
  FOREIGN KEY (set_id) REFERENCES sets(id)
);