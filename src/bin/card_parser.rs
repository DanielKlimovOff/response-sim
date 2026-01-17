use reqwest::{Client, Response};
use tokio::time::{sleep, Duration};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use sqlx::SqlitePool;

#[serde_as]
#[derive(Deserialize, Debug)]
struct CardJson {
    name: String,
    images: serde_json::Value,
    #[serde_as(as = "DisplayFromStr")]
    id_in_set: u32,
    rarity: RarityJson,
    // fractions: Vec<FractionJson>,
    types: Vec<CardTypeJson>,
    card_set: SetJson,
}

#[derive(Deserialize, Debug)]
struct RarityJson {
    display_name: String,
}

// #[derive(Deserialize, Debug)]
// struct FractionJson {
//     display_name: String,
// }

#[derive(Deserialize, Debug)]
struct CardTypeJson {
    display_name: String,
}

#[derive(Deserialize, Debug)]
struct SetJson {
    display_name: String,
}

#[derive(Debug)]
struct Card {
    name: String,
    id_in_set: u32,
    rarity: CardRarity,
    buster_slot: BusterSlot,
    set: CardSet,
    image_url: Option<String>,
}


impl Card {
    fn from_json(card_json: CardJson) -> Result<Self, String> {
        let rarity = match card_json.rarity.display_name.as_str() {
            "Бронза" => CardRarity::Bronze,
            "Серебро" => CardRarity::Silver,
            "Золото" => CardRarity::Gold,
            _ => return Err(format!("Unknown rarity name: {}", card_json.rarity.display_name)),
        };

        // let fractions = card_json.fractions.iter().map(|fraction_json| {
        //     match fraction_json.display_name.as_str() {
        //         "Жизнь" => Ok(Fraction::Life),
        //         "Порядок" => Ok(Fraction::Justice),
        //         "Природа" => Ok(Fraction::Nature),
        //         "Смерть" => Ok(Fraction::Death),
        //         "Технологии" => Ok(Fraction::Technology),
        //         "Хаос" => Ok(Fraction::Chaos),
        //         _ => Err(format!("Unknown fraction name: {}", fraction_json.display_name)),
        //     }
        // }).collect::<Result<Vec<_>, String>>()?;

        let buster_slot = if card_json.types.iter().any(|t| t.display_name.as_str() == "Герой") {
            BusterSlot::Hero
        } else if card_json.types.iter().any(|t| t.display_name.as_str() == "Приказ") {
            BusterSlot::Command
        } else {
            BusterSlot::BasicCard
        };

        let set = match card_json.card_set.display_name.as_str() {
            "БАЗ" => CardSet::BAZ,
            "КОВ" => CardSet::KOV,
            "Зал Славы" => CardSet::HallOfFame,
            _ => return Err(format!("Unknown set name: {}", card_json.card_set.display_name).into()),
        };

        let image_url = card_json.images.get("card_path")
            .and_then(|path| path.as_str().map(|s| RESPONSE_WORLD_URL.to_string() + s));

        Ok(Card {
            name: card_json.name,
            id_in_set: card_json.id_in_set,
            rarity,
            // fractions,
            buster_slot,
            set,
            image_url,
        })
    }
}

#[derive(Debug)]
enum CardRarity {
    Bronze,
    Silver,
    Gold,
}

impl CardRarity {
    fn as_str(&self) -> &str {
        match self {
            CardRarity::Bronze => "Бронза",
            CardRarity::Silver => "Серебро",
            CardRarity::Gold => "Золото",
        }
    }
}

// #[derive(Debug)]
// enum Fraction {
//     Life,
//     Justice,
//     Nature,
//     Death,
//     Technology,
//     Chaos,
// }

// impl Fraction {
//     fn as_str(&self) -> &str {
//         match self {
//             Fraction::Life => "Жизнь",
//             Fraction::Justice => "Порядок",
//             Fraction::Nature => "Природа",
//             Fraction::Death => "Смерть",
//             Fraction::Technology => "Технологии",
//             Fraction::Chaos => "Хаос",
//         }
//     }
// }

#[derive(Debug)]
enum BusterSlot {
    Hero,
    Command,
    BasicCard,
}

impl BusterSlot {
    fn as_str(&self) -> &str {
        match self {
            BusterSlot::Hero => "Герой",
            BusterSlot::Command => "Приказ",
            BusterSlot::BasicCard => "Основная карта",
        }
    }
}

#[derive(Debug)]
enum CardSet {
    BAZ,
    KOV,
    HallOfFame,
}

impl CardSet {
    fn as_str(&self) -> &str {
        match self {
            CardSet::BAZ => "БАЗ",
            CardSet::KOV => "КОВ",
            CardSet::HallOfFame => "Зал Славы",
        }
    }
}

const LAST_CARD_ID: u32 = 415;
const RESPONSE_WORLD_URL: &str = "https://response-world.ru/";
const REQUEST_DELAY_MS: u64 = 100;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = SqlitePool::connect(&std::env::var("DATABASE_URL")?).await?;
    let mut have_problem = false;
    let base_url = RESPONSE_WORLD_URL.to_string() + "api/cards/";
    for card_id in 1..=LAST_CARD_ID {
        let url = format!("{}{}", base_url, card_id);
        let card_json_value: serde_json::Value = match get_with_retry(&url).await {
            Ok(response) => {
                println!("CARD #{} DONE", card_id);
                response.json().await?
            }
            Err(e) => {
                println!("CARD #{} FAILED\nERROR: {}", card_id, e);
                have_problem = true;
                continue;
            }
        };
        let card_json: CardJson = serde_json::from_value(card_json_value["card"].clone())?;
        let card = match Card::from_json(card_json) {
            Ok(card) => {
                println!("Parsed card: {:?}", card);
                card
            }
            Err(e) => {
                println!("Failed to parse card #{}: {}", card_id, e);
                have_problem = true;
                continue;
            }
        };
        insert_card_into_db(&pool, &card).await?;
        sleep(Duration::from_millis(REQUEST_DELAY_MS)).await;
    }

    if have_problem {
        println!("Some cards failed to process.");
    } else {
        println!("All cards processed successfully.");
    }
    Ok(())
}

async fn insert_card_into_db(pool: &SqlitePool, card: &Card) -> Result<(), Box<dyn std::error::Error>> {
    let rarity = card.rarity.as_str();
    let buster_slot = card.buster_slot.as_str();
    let set = card.set.as_str();
    sqlx::query!(
        "INSERT 
            INTO cards (id, name, rarity_id, type_id, set_id, id_in_set, image_url) 
            SELECT 
                (SELECT COALESCE(MAX(id), 0) + 1 FROM cards),
                ?,
                (SELECT id FROM rarities WHERE name = ?),
                (SELECT id FROM types WHERE name = ?),
                (SELECT id FROM sets WHERE short_name = ?),
                ?,
                ?
            WHERE NOT EXISTS (
                SELECT 1 FROM cards WHERE name = ? AND set_id = (SELECT id FROM sets WHERE short_name = ?)
            );",
        card.name,
        rarity,
        buster_slot,
        set,
        card.id_in_set,
        card.image_url,
        card.name,
        set,
    ).execute(pool).await?;
    Ok(())
}

async fn get_with_retry(url: &str) -> Result<Response, reqwest::Error> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let max_retries = 3;
    let mut attempt = 0;

    loop {
        attempt += 1;

        match client.get(url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    return Ok(resp);
                } else if resp.status().is_server_error() && attempt < max_retries {
                    // 5xx — можно ретраить
                } else {
                    return Err(resp.error_for_status().unwrap_err());
                }
            }
            Err(err) => {
                if attempt >= max_retries {
                    return Err(err);
                }

                // timeout / network error
                if err.is_timeout() || err.is_connect() {
                    // retry
                } else {
                    return Err(err);
                }
            }
        }

        // задержка перед повтором
        sleep(Duration::from_secs(1)).await;
    }
}