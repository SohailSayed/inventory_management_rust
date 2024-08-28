use futures::executor::block_on;
use sea_orm::{Database, DbErr};

// hard-coding password for now, would not do this in production!
const DATABASE_URL: &str = "postgres://postgres:password123@db:5432";
const DB_NAME: &str = "warehouse_db";

async fn run() -> Result<(), DbErr> {
    let db = Database::connect(DATABASE_URL).await?;

    Ok(())
}

fn main() {
    println!("Starting");
    if let Err(err) = block_on(run()) {
        panic!("{}", err);
    }
    else {
        println!("Connected!");
    }
}
