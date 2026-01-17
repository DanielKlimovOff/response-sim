-- Add migration script here
INSERT INTO rarities (id, name) VALUES
  (1, 'Бронза'),
  (2, 'Серебро'),
  (3, 'Золото');

INSERT INTO types (id, name) VALUES
  (1, 'Герой'),
  (2, 'Приказ'),
  (3, 'Основная карта');

INSERT INTO sets (id, name, short_name, release_date, card_count) VALUES
  (1, 'Базовый', 'БАЗ', '2025-10-21', 132),
  (2, 'Королевства Ванстера', 'КОВ', '2025-10-21', 273),
  (3, 'Зал Славы', 'Зал Славы', '2025-10-21', 10);