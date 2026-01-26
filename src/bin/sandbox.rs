use response_sim::*;
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_pool = SqlitePool::connect(&std::env::var("DATABASE_URL")?).await?;
    let mut card_base = CardBase::new(db_pool).await;
    let hall_of_fame_chanse = 0.02;
    let hero_chanse = Distribution::new([0.8, 0.2]).unwrap();
    let command_chanse_bronze_silver = Distribution::new([0.75, 0.25]).unwrap();
    let command_chanse_random = Distribution::new([0.7, 0.2, 0.1]).unwrap();
    let basic_card_chanse = Distribution::new([0.7, 0.3]).unwrap();
    let buster_rules = BusterRules::new(
        CardSet::KOV,
        hall_of_fame_chanse,
        hero_chanse,
        command_chanse_bronze_silver,
        command_chanse_random,
        basic_card_chanse,
    ).unwrap();
    let mut nums = Vec::new();
    for _ in 0..1000 {
        let mut again = true;
        let mut i = 0;
        while again {
            i += 1;
            // println!("BUSTER #{i}");
            // println!("---------------------------------------");
            card_base.generate_buster(&buster_rules).await.unwrap().iter().for_each(|c| {
                // println!("{}", c);
                if c.set == CardSet::HallOfFame {
                    again = false;
                }
            });
        }
        nums.push(i);
    }
    println!("{:#?}", nums);
    println!("{}", nums.iter().sum::<usize>() / nums.len());
    Ok(())
}