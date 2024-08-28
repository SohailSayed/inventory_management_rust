mod migrator;
mod entities;

use futures::executor::block_on;
use sea_orm::*;
use sea_orm_migration::prelude::*;
use entities::{prelude::*, *};

use crate::migrator::Migrator;

// hard-coding password for now, would not do this in production!
const DATABASE_URL: &str = "postgres://postgres:password123@db:5432";
const DB_NAME: &str = "warehouse_db";

async fn run() -> Result<(), DbErr> {
    let db = Database::connect(DATABASE_URL).await?;

    let db = &match db.get_database_backend() {
        DbBackend::MySql => db,
        DbBackend::Postgres => {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("DROP DATABASE IF EXISTS \"{}\";", DB_NAME),
            ))
            .await?;
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE \"{}\";", DB_NAME),
            ))
            .await?;
 
            let url = format!("{}/{}", DATABASE_URL, DB_NAME);
            Database::connect(&url).await?
        }
        DbBackend::Sqlite => db,
    };

    let schema_manager = SchemaManager::new(db);

    Migrator::refresh(db).await?;
    assert!(schema_manager.has_table("product").await?);
    assert!(schema_manager.has_table("inventory").await?);

    // create
    create_product(db, "Sample Product 2", 20.0).await?;
    // read
    find_product_by_id(db, 1).await?;
    find_product_by_name(db, "Sample Product 2").await?;
    // update
    update_product(db, 1, "Updated Product Name", 30.0).await?;
    // delete
    delete_product(db, 1).await?;

    Ok(())
}

async fn create_product(db: &DatabaseConnection, name: &str, price: f64) -> Result<(), DbErr> {
    let new_product = product::ActiveModel {
        name: ActiveValue::Set(name.to_owned()),
        price: ActiveValue::Set(price),
        ..Default::default()
    };
    let res = Product::insert(new_product).exec(db).await?;
    
    Ok(())
}

async fn find_product_by_id(db: &DatabaseConnection, id: i32) -> Result<(), DbErr> {
    let found_product: Option<product::Model> = Product::find_by_id(id).one(db).await?;
    println!("{}", found_product.unwrap().name);

    Ok(()) 
}  

async fn find_product_by_name(db: &DatabaseConnection, name: &str) -> Result<(), DbErr> {
    // for testing - need to enforce unique names for each product somewhere
    let found_product: Option<product::Model> = Product::find()
    .filter(product::Column::Name.eq(name.to_owned()))
    .one(db)
    .await?;
    println!("{}", found_product.unwrap().name);

    Ok(()) 
}

async fn update_product(db: &DatabaseConnection, id: i32, name: &str, price: f64) -> Result<(), DbErr> {
    let updated_product = product::ActiveModel {
        id: ActiveValue::Set(id),
        name: ActiveValue::Set(name.to_owned()),
        price: ActiveValue::Set(price),
    };
    updated_product.update(db).await?;
    
    Ok(())
}

async fn delete_product(db: &DatabaseConnection, id: i32) -> Result<(), DbErr> {
    let deleted_product = product::ActiveModel {
        id: ActiveValue::Set(id),
        ..Default::default()
    };
    deleted_product.delete(db).await?;
    
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
