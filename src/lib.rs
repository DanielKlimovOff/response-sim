use std::collections::HashMap;

use rand::{Rng, seq::IndexedRandom};
use sqlx::SqlitePool;

const BUSTER_SIZE: usize = 18;

#[derive(Debug, Clone)]
struct Card {
    name: String,
    id_in_set: u32,
    rarity: CardRarity,
    buster_slot: CardBusterSlot,
    set: CardSet,
    image_url: Option<String>,
}

impl Card {
    /// Создает карту "пустышку", перед использованием нужно заполнить поля
    fn plug() -> Self {
        Card {
            name: "Пустышка".to_string(),
            id_in_set: 0,
            rarity: CardRarity::Bronze,
            buster_slot: CardBusterSlot::BasicCard,
            set: CardSet::BAZ,
            image_url: None,
        }
    }

    fn new(name: String, id_in_set: u32, rarity: CardRarity, buster_slot: CardBusterSlot, set: CardSet, image_url: Option<String>) -> Self {
        Card {
            name,
            id_in_set,
            rarity,
            buster_slot,
            set,
            image_url,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
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

impl TryFrom<String> for CardRarity {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "Бронза" => Ok(CardRarity::Bronze),
            "Серебро" => Ok(CardRarity::Silver),
            "Золото" => Ok(CardRarity::Gold),
            r => Err(format!("Error. Not found rarity: {}", r).into())
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
enum CardBusterSlot {
    Hero,
    Command,
    BasicCard,
}

impl CardBusterSlot {
    fn as_str(&self) -> &str {
        match self {
            CardBusterSlot::Hero => "Герой",
            CardBusterSlot::Command => "Приказ",
            CardBusterSlot::BasicCard => "Основная карта",
        }
    }

    fn iter() -> impl Iterator<Item = CardBusterSlot> {
        [CardBusterSlot::Hero, CardBusterSlot::Command, CardBusterSlot::BasicCard].into_iter()
    }
}

impl TryFrom<String> for CardBusterSlot {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "Герой" => Ok(CardBusterSlot::Hero),
            "Приказ" => Ok(CardBusterSlot::Command),
            "Основная карта" => Ok(CardBusterSlot::BasicCard),
            s => Err(format!("Error. Not found buster slot: {}", s).into())
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
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

impl TryFrom<String> for CardSet {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "БАЗ" => Ok(CardSet::BAZ),
            "КОВ" => Ok(CardSet::KOV),
            "Зал Славы" => Ok(CardSet::HallOfFame),
            s => Err(format!("Error. Not found set: {}", s).into())
        }
    }
}

#[derive(Debug)]
struct CardBase {
    cards: HashMap<(CardBusterSlot, CardSet), Vec<Card>>,
    db_pull: SqlitePool,
    rng: rand::rngs::ThreadRng,
}

impl CardBase {
    async fn new(db_pull: SqlitePool) -> Self {
        let rng = rand::rng();
        let mut new_card_base = CardBase {
            cards: HashMap::new(),
            db_pull: db_pull,
            rng,
        };
        new_card_base.add_set(CardSet::HallOfFame).await;
        new_card_base
    }

    fn has_set(&self, set: CardSet) -> bool {
        CardBusterSlot::iter().any(|slot| self.cards.contains_key(&(slot, set)))
    }

    async fn add_set(&mut self, set: CardSet) -> Result<(), Box<dyn std::error::Error>>{
        CardBusterSlot::iter().for_each(|slot| { self.cards.insert((slot, set), Vec::new()); } );
            
        let set_str = set.as_str();
        sqlx::query!(
            "SELECT cards.name as name, id_in_set, rarities.name as rarity, types.name as buster_slot, sets.short_name as set_name, image_url
            FROM cards
            INNER JOIN rarities ON cards.rarity_id = rarities.id
            INNER JOIN types ON cards.type_id = types.id
            INNER JOIN sets ON cards.set_id = sets.id
            WHERE set_id = (SELECT id from sets WHERE short_name = ?)",
            set_str,
            ).fetch_all(&self.db_pull)
            .await?
            .into_iter()
            .map(|rec| {
                let rarity = CardRarity::try_from(rec.rarity)?;
                let buster_slot = CardBusterSlot::try_from(rec.buster_slot)?;
                let set = CardSet::try_from(rec.set_name)?;
                Ok(Card::new(rec.name, u32::try_from(rec.id_in_set)?, rarity, buster_slot, set, rec.image_url))
            })
            .collect::<Result<Vec<Card>, Box<dyn std::error::Error>>>()?
            .into_iter().for_each(|c| self.cards.get_mut(&(c.buster_slot, set)).unwrap().push(c.clone()));

        Ok(())
    }

    async fn generate_card(&mut self, slot: CardBusterSlot, rarity: CardRarity, set: CardSet, hall_of_fame: bool) -> Option<Card> {
        if hall_of_fame {
            if let Some(card) = self.cards[&(slot, CardSet::HallOfFame)].choose(&mut self.rng) {
                Some(card.clone())
            } else {
                Some(self.cards[&(slot, set)].choose(&mut self.rng)?.clone())
            }
        } else {
            Some(self.cards[&(slot, set)].choose(&mut self.rng)?.clone())
        }
    }

    async fn generate_buster(&mut self, rules: &BusterRules) -> Option<[Card; BUSTER_SIZE]> {
        if !self.has_set(rules.set) {
            self.add_set(rules.set).await;
        }

        let mut buster = std::array::from_fn(|_| Card::plug());

        // Герой серебро/золото
        let hero_rarity = match rules.hero_chanse.generate() {
            0 => CardRarity::Silver,
            1 => CardRarity::Gold,
            _ => unreachable!(),
        };
        buster[0] = self.generate_card(CardBusterSlot::Hero, hero_rarity, rules.set, rules.generate_hall_of_fame()).await?;

        // Приказ случайно редкости
        let command_rarity = match rules.command_chanse_random.generate() {
            0 => CardRarity::Bronze,
            1 => CardRarity::Silver,
            2 => CardRarity::Gold,
            _ => unreachable!(),
        };
        buster[1] = self.generate_card(CardBusterSlot::Command, command_rarity, rules.set, rules.generate_hall_of_fame()).await?;

        // Приказ серебро/золото
        let command_rarity = match rules.command_chanse_bronze_silver.generate() {
            0 => CardRarity::Silver,
            1 => CardRarity::Gold,
            _ => unreachable!(),
        };
        buster[2] = self.generate_card(CardBusterSlot::Command, command_rarity, rules.set, rules.generate_hall_of_fame()).await?;

        // Приказ бронза
        buster[3] = self.generate_card(CardBusterSlot::Command, CardRarity::Bronze, rules.set, rules.generate_hall_of_fame()).await?;

        // Основная карта золото
        buster[4] = self.generate_card(CardBusterSlot::BasicCard, CardRarity::Gold, rules.set, rules.generate_hall_of_fame()).await?;

        // Основная карта серебро/золото
        let basic_card_rarity = match rules.basic_card_chanse.generate() {
            0 => CardRarity::Silver,
            1 => CardRarity::Gold,
            _ => unreachable!(),
        };
        buster[5] = self.generate_card(CardBusterSlot::BasicCard, basic_card_rarity, rules.set, rules.generate_hall_of_fame()).await?;

        // Основная карта серебро
        buster[6] = self.generate_card(CardBusterSlot::BasicCard, CardRarity::Silver, rules.set, rules.generate_hall_of_fame()).await?;

        // Основные карты бронза
        for i in 7..BUSTER_SIZE {
            buster[i] = self.generate_card(CardBusterSlot::BasicCard, CardRarity::Bronze, rules.set, rules.generate_hall_of_fame()).await?;
        }   

        Some(buster)
    }
}

#[derive(Debug)]
struct BusterRules {
    set: CardSet,
    hall_of_fame_chanse: f64,
    hero_chanse: Distribution<2>,
    command_chanse_bronze_silver: Distribution<2>,
    command_chanse_random: Distribution<3>,
    basic_card_chanse: Distribution<2>,
}

impl BusterRules {
    fn new(
        set: CardSet,
        hall_of_fame_chanse: f64,
        hero_chanse: Distribution<2>,
        command_chanse_bronze_silver: Distribution<2>,
        command_chanse_random: Distribution<3>,
        basic_card_chanse: Distribution<2>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if hall_of_fame_chanse < 0.0 || hall_of_fame_chanse > 1.0 {
            return Err("hall_of_fame_chanse must be between 0.0 and 1.0".into());
        }
        Ok(BusterRules {
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse_bronze_silver,
            command_chanse_random,
            basic_card_chanse,
        })
    }

    fn generate_hall_of_fame(&self) -> bool {
        let mut rng = rand::rng();
        rng.random::<f64>() < self.hall_of_fame_chanse
    }
}

#[derive(Debug)]
struct Distribution<const SIZE: usize> {
    values: [f64; SIZE],
}

impl<const SIZE: usize> Distribution<SIZE> {
    fn new(values: [f64; SIZE]) -> Result<Distribution<SIZE>, Box<dyn std::error::Error>> {
        if values.iter().any(|&v| v < 0.0 || v > 1.0) {
            return Err("Distribution values must be between 0.0 and 1.0".into());
        }
        if (1.0 - values.iter().sum::<f64>()).abs() > f64::EPSILON {
            return Err("Sum of distribution values must be 1.0".into());
        }
        Ok(Distribution {
            values,
        })
    }

    fn values(&self) -> &[f64; SIZE] {
        &self.values
    }

    /// Возвращает usize от 0 до SIZE-1 в соответствии с распределением
    fn generate(&self) -> usize {
        let mut rng = rand::rng();
        let random_value: f64 = rng.random();
        let mut cumulative_sum = 0.0;
        for (index, &value) in self.values().iter().enumerate() {
            cumulative_sum += value;
            if random_value <= cumulative_sum {
                return index;
            }
        }
        SIZE - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_distribution_when_valid_values_then_ok1() {
        // GIVEN
        let my_distribution = [0.2, 0.5, 0.3];
        // WHEN
        let result = Distribution::<3>::new(my_distribution);
        // THEN
        assert!(result.is_ok());
    }

    #[test]
    fn create_distribution_when_valid_values_then_ok2() {
        // GIVEN
        let my_distribution = [1.0, 0.0, 0.0, 0.0];
        // WHEN
        let result = Distribution::<4>::new(my_distribution);
        // THEN
        assert!(result.is_ok());
    }

    #[test]
    fn create_distribution_when_valid_values_then_ok3() {
        // GIVEN
        let my_distribution = [0.001, 0.999];
        // WHEN
        let result = Distribution::<2>::new(my_distribution);
        // THEN
        assert!(result.is_ok());
    }

    #[test]
    fn create_distribution_when_wrong_sum_then_err1() {
        // GIVEN
        let my_distribution = [0.5, 0.5, 0.5, 0.5];
        // WHEN
        let result = Distribution::<4>::new(my_distribution);
        // THEN
        assert!(result.is_err());
    }

    #[test]
    fn create_distribution_when_wrong_sum_then_err2() {
        // GIVEN
        let my_distribution = [0.2, 0.1, 0.2, 0.1];
        // WHEN
        let result = Distribution::<4>::new(my_distribution);
        // THEN
        assert!(result.is_err());
    }

    #[test]
    fn create_distribution_when_wrong_element_then_err1() {
        // GIVEN
        let my_distribution = [0.2, 20.0, 0.2, 0.1];
        // WHEN
        let result = Distribution::<4>::new(my_distribution);
        // THEN
        assert!(result.is_err());
    }

    #[test]
    fn create_distribution_when_wrong_element_then_err2() {
        // GIVEN
        let my_distribution = [0.1, 0.2, 0.3, -0.4];
        // WHEN
        let result = Distribution::<4>::new(my_distribution);
        // THEN
        assert!(result.is_err());
    }

    #[test]
    fn get_values_when_valid_values_then_ok() {
        // GIVEN
        let my_distribution = [0.1, 0.2, 0.3, 0.4];
        // WHEN
        let result = Distribution::<4>::new(my_distribution);
        let d = result.unwrap();
        // THEN
        assert_eq!(d.values(), &[0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn generate_with_guaranteed_distribution1() {
        // GIVEN
        let my_distribution = [1.0, 0.0, 0.0, 0.0, 0.0];
        // WHEN
        let result = Distribution::<5>::new(my_distribution);
        let d = result.unwrap();
        let value = d.generate();
        // THEN
        assert_eq!(value, 0);
    }

    #[test]
    fn generate_with_guaranteed_distribution2() {
        // GIVEN
        let my_distribution = [0.0, 0.0, 1.0, 0.0, 0.0];
        // WHEN
        let result = Distribution::<5>::new(my_distribution);
        let d = result.unwrap();
        let value = d.generate();
        // THEN
        assert_eq!(value, 2);
    }

    #[test]
    fn generate_with_guaranteed_distribution3() {
        // GIVEN
        let my_distribution = [0.0, 0.0, 0.0, 0.0, 1.0];
        // WHEN
        let result = Distribution::<5>::new(my_distribution);
        let d = result.unwrap();
        let value = d.generate();
        // THEN
        assert_eq!(value, 4);
    }

    #[test]
    fn generate_with_random_distribution() {
        // GIVEN
        let my_distribution = [0.2, 0.2, 0.2, 0.2, 0.2];
        // WHEN
        let result = Distribution::<5>::new(my_distribution);
        let d = result.unwrap();
        let value = d.generate();
        // THEN
        assert!(value <= 4);
    }

    #[test]
    fn create_buster_rules_with_valid_values() {
        // GIVEN
        let hall_of_fame_chanse = 0.1;
        let my_distribution2 = [0.2, 0.8];
        let my_distribution3 = [0.2, 0.4, 0.4];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        );
        // THEN
        assert!(rules.is_ok());
    }

    #[test]
    fn create_buster_rules_with_invalid_hall_of_fame_chanse1() {
        // GIVEN
        let hall_of_fame_chanse = 1.001;
        let my_distribution2 = [0.2, 0.8];
        let my_distribution3 = [0.2, 0.4, 0.4];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        );
        // THEN
        assert!(rules.is_err());
    }

    #[test]
    fn create_buster_rules_with_invalid_hall_of_fame_chanse2() {
        // GIVEN
        let hall_of_fame_chanse = -0.1;
        let my_distribution2 = [0.2, 0.8];
        let my_distribution3 = [0.2, 0.4, 0.4];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        );
        // THEN
        assert!(rules.is_err());
    }

    #[test]
    fn generate_buster_check_hero() {
        let buster = standart_buster_when_given();
        // THEN
        assert_eq!(buster[0].buster_slot, CardBusterSlot::Hero);
    }

    #[test]
    fn generate_buster_check_commands() {
        let buster = standart_buster_when_given();
        // THEN
        for i in 1..=3 {
            assert_eq!(buster[i].buster_slot, CardBusterSlot::Command);
        }
    }

    #[test]
    fn generate_buster_check_basic_cards() {
        let buster = standart_buster_when_given();
        // THEN
        for i in 4..BUSTER_SIZE {
            assert_eq!(buster[i].buster_slot, CardBusterSlot::BasicCard);
        }
    }

    #[test]
    fn generate_buster_check_guaranteed_rarities() {
        let buster = standart_buster_when_given();
        // THEN
        assert_ne!(buster[0].rarity, CardRarity::Bronze);
        assert_eq!(buster[2].rarity, CardRarity::Gold);
        assert_eq!(buster[3].rarity, CardRarity::Bronze);
        assert_eq!(buster[4].rarity, CardRarity::Gold);
        assert_ne!(buster[5].rarity, CardRarity::Bronze);
        assert_eq!(buster[6].rarity, CardRarity::Silver);
        for i in 7..BUSTER_SIZE {
            assert_eq!(buster[i].rarity, CardRarity::Bronze);
        }   
    }

    #[test]
    fn generate_buster_check_random_rarities_with_1_chanse() {
        // GIVEN
        let hall_of_fame_chanse = 0.5;
        let my_distribution2 = [0.0, 1.0];
        let my_distribution3 = [0.0, 0.0, 1.0];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        ).unwrap();
        let buster = rules.generate_buster().unwrap();
        // THEN
        assert_eq!(buster[0].rarity, CardRarity::Gold);
        assert_eq!(buster[1].rarity, CardRarity::Gold);
        assert_eq!(buster[2].rarity, CardRarity::Silver);
        assert_eq!(buster[5].rarity, CardRarity::Gold);
    }

    #[test]
    fn generate_buster_check_random_rarities_with_0_chanse() {
        // GIVEN
        let hall_of_fame_chanse = 0.5;
        let my_distribution2 = [1.0, 0.0];
        let my_distribution3 = [1.0, 0.0, 0.0];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        ).unwrap();
        let buster = rules.generate_buster().unwrap();
        // THEN
        assert_eq!(buster[0].rarity, CardRarity::Silver);
        assert_eq!(buster[1].rarity, CardRarity::Bronze);
        assert_eq!(buster[2].rarity, CardRarity::Bronze);
        assert_eq!(buster[5].rarity, CardRarity::Silver);
    }

    #[test]
    fn generate_buster_check_hall_of_fame_with_1_chanse() {
        // GIVEN
        let hall_of_fame_chanse = 1.0;
        let my_distribution2 = [0.0, 1.0];
        let my_distribution3 = [0.0, 0.0, 1.0];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        ).unwrap();
        let buster = rules.generate_buster().unwrap();
        // THEN
        assert_eq!(buster[0].set, CardSet::HallOfFame);
        assert_eq!(buster[4].set, CardSet::HallOfFame);
        assert_eq!(buster[5].set, CardSet::HallOfFame);
    }

    #[test]
    fn generate_buster_check_no_hall_of_fame_with_0_chanse() {
        // GIVEN
        let hall_of_fame_chanse = 0.0;
        let my_distribution2 = [0.5, 0.5];
        let my_distribution3 = [0.8, 0.15, 0.05];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        ).unwrap();
        let buster = rules.generate_buster().unwrap();
        // THEN
        for card in buster.iter() {
            assert_ne!(card.set, CardSet::HallOfFame);
        }
    }

    #[test]
    fn generate_card_with_valid_values() {
        // GIVEN
        let slot = CardBusterSlot::Hero;
        let rarity = CardRarity::Silver;
        let set = CardSet::KOV;
        let hall_of_fame = false;
        // WHEN
        let card = generate_card(slot, rarity, set, hall_of_fame);
        // THEN
        assert!(card.is_some());
    }

    #[test]
    fn generate_card_with_valid_values_then_card_is_legal() {
        // GIVEN
        let slot = CardBusterSlot::Hero;
        let rarity = CardRarity::Silver;
        let set = CardSet::KOV;
        let hall_of_fame = false;
        // WHEN
        let card = generate_card(slot, rarity, set, hall_of_fame).unwrap();
        // THEN
        assert_eq!(card.buster_slot, CardBusterSlot::Hero);
        assert_eq!(card.rarity, CardRarity::Silver);
        assert_eq!(card.set, CardSet::KOV);
    }

    /// На январь 2026 в Зале Славы нет приказов
    #[test]
    fn generate_card_with_hall_of_fame_command() {
        // GIVEN
        let slot = CardBusterSlot::Command;
        let rarity = CardRarity::Gold;
        let set = CardSet::HallOfFame;
        let hall_of_fame = true;
        // WHEN
        let card = generate_card(slot, rarity, set, hall_of_fame);
        // THEN
        assert!(card.is_none());
    }

    /// На январь 2026 в Зале Славы все карты золотой редкости
    #[test]
    fn generate_card_with_hall_of_fame_silver() {
        // GIVEN
        let slot = CardBusterSlot::BasicCard;
        let rarity = CardRarity::Silver;
        let set = CardSet::HallOfFame;
        let hall_of_fame = true;
        // WHEN
        let card = generate_card(slot, rarity, set, hall_of_fame);
        // THEN
        assert!(card.is_none());
    }

    #[test]
    fn generate_card_with_hall_of_fame() {
        // GIVEN
        let slot = CardBusterSlot::BasicCard;
        let rarity = CardRarity::Gold;
        let set = CardSet::KOV;
        let hall_of_fame = true;
        // WHEN
        let card = generate_card(slot, rarity, set, hall_of_fame).unwrap();
        // THEN
        assert_eq!(card.set, CardSet::HallOfFame);
    }

    #[test]
    fn generate_card_without_hall_of_fame() {
        // GIVEN
        let slot = CardBusterSlot::BasicCard;
        let rarity = CardRarity::Gold;
        let set = CardSet::KOV;
        let hall_of_fame = false;
        // WHEN
        let card = generate_card(slot, rarity, set, hall_of_fame).unwrap();
        // THEN
        assert_eq!(card.set, CardSet::KOV);
    }

    fn standart_buster_when_given() -> [Card; BUSTER_SIZE] {
        // GIVEN
        let hall_of_fame_chanse = 0.5;
        let my_distribution2 = [0.5, 0.5];
        let my_distribution3 = [0.8, 0.15, 0.05];
        let set = CardSet::KOV;
        // WHEN
        let hero_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse2 = Distribution::<2>::new(my_distribution2).unwrap();
        let command_chanse3 = Distribution::<3>::new(my_distribution3).unwrap();
        let basic_card_chanse = Distribution::<2>::new(my_distribution2).unwrap();
        let rules = BusterRules::new(
            set,
            hall_of_fame_chanse,
            hero_chanse,
            command_chanse2,
            command_chanse3,
            basic_card_chanse,
        ).unwrap();
        let buster = rules.generate_buster();
        buster.unwrap()
    }
}
