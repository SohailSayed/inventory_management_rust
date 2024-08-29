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
// arbritary threshold of 30% picked to flag low stock products
const LOW_THRESHOLD: f64 = 0.3;

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
    create_product(db, "Sample Product 2", 20.0, 100).await?;
    // read
    find_product_by_id(db, 1).await?;
    find_product_by_name(db, "Sample Product 2").await?;
    // update
    update_product(db, 1, "Updated Product Name", 30.0).await?;
    // delete
    delete_product(db, 1).await?;

    // find inventory by name
    // update quantity
    create_product(db, "Sample Product 3", 55.0, 300).await?;
    update_inventory_quantity(db, "Sample Product 3", 151).await?; 

    create_product(db, "Sample Product 4", 55.0, 20).await?;
    update_inventory_quantity(db, "Sample Product 4", 1).await?; 

    create_product(db, "Sample Product 5", 55.0, 200).await?;
    update_inventory_quantity(db, "Sample Product 5", 3).await?; 

    // retrieve low stock
    retrieve_low_stock(db, LOW_THRESHOLD).await?;
    // calculate total inventory value
    calculate_total_inventory_value(db).await?;

    Ok(())
}

async fn calculate_total_inventory_value(db: &DatabaseConnection) -> Result<f64, DbErr> {
    let inventory: Vec<inventory::Model> = Inventory::find().all(db).await?;
    let mut total_value: f64 = 0.0;
    for product in &inventory {
        let product_id = product.product_id;
        let quantity = product.quantity;
        let price = find_product_by_id(db, product_id).await?.unwrap().price;
        let product_value = f64::from(quantity) * price;
        total_value += product_value;
    }
    println!("Total inventory value: ${}", total_value);
    Ok(total_value)
}

async fn retrieve_low_stock(db: &DatabaseConnection, threshold: f64) -> Result<Vec<inventory::Model>, DbErr> {
    if threshold > 1.00 {
        return Err(DbErr::Custom("Threshold can't exceed 1.00 (100%)".to_owned()));
    }
    let low_stock_products: Vec<inventory::Model> = Inventory::find()
        .filter(
            Condition::all()
                .add(inventory::Column::Stock.lte(threshold))
        )
        .all(db)
        .await?;

    for product in &low_stock_products {
        println!("Low Stock Products: {}", product.name);
    }
    Ok(low_stock_products)
}

async fn create_product(db: &DatabaseConnection, name: &str, price: f64, capacity: i32) -> Result<(product::Model, inventory::Model), DbErr> {
    if capacity == 0 {
        return Err(DbErr::Custom("Capacity can't be zero.".to_owned()));
    }
    if capacity < 0 {
        return Err(DbErr::Custom("Capacity can't be negative.".to_owned()));
    }
    if price < 0.0 {
        return Err(DbErr::Custom("Price can't be negative.".to_owned()));
    }
    let new_product = product::ActiveModel {
        name: ActiveValue::Set(name.to_owned()),
        price: ActiveValue::Set(price),
        ..Default::default()
    };
    let product_result = Product::insert(new_product).exec(db).await?;

    // reflect change in inventory
    let new_inventory = inventory::ActiveModel {
        name: ActiveValue::Set(name.to_owned()),
        quantity: ActiveValue::Set(capacity),
        capacity: ActiveValue::Set(capacity),
        stock: ActiveValue::Set(1.0),
        product_id: ActiveValue::Set(product_result.last_insert_id),
        ..Default::default()
    };
    let inventory_result = Inventory::insert(new_inventory).exec(db).await?;
    Ok((
        product::Model {
            id: product_result.last_insert_id,
            name: name.to_owned(),
            price: price,
        },
        inventory::Model {
            id: inventory_result.last_insert_id,
            name: name.to_owned(),
            quantity: capacity,
            capacity: capacity,
            stock: 1.0,
            product_id: product_result.last_insert_id,
        }
    ))
}

async fn find_product_by_id(db: &DatabaseConnection, id: i32) -> Result<Option<product::Model>, DbErr> {
    let found_product: Option<product::Model> = Product::find_by_id(id).one(db).await?;
    if let None = found_product {
        return Err(DbErr::Custom(format!("Product with ID {} not found", id)));
    }
    println!("{}", found_product.as_ref().unwrap().name);
    Ok(found_product) 
}  

async fn fetch_inventory_by_product_id(db: &DatabaseConnection, product_id: i32) -> Result<i32, DbErr> {
    let fetched_inventory: Option<inventory::Model> = Inventory::find()
    .filter(inventory::Column::ProductId.eq(product_id))
    .one(db)
    .await?;
    if let None = fetched_inventory {
        return Err(DbErr::Custom(format!("Inventory with Product ID {} not found", product_id)));
    }
    println!("{}", fetched_inventory.as_ref().unwrap().id);
    Ok(fetched_inventory.unwrap().id)
}

async fn find_product_by_name(db: &DatabaseConnection, name: &str) -> Result<Option<product::Model>, DbErr> {
    // for testing - need to enforce unique names for each product somewhere
    let found_product: Option<product::Model> = Product::find()
    .filter(product::Column::Name.eq(name.to_owned()))
    .one(db)
    .await?;
    if let None = found_product {
        return Err(DbErr::Custom(format!("Product with name {} not found", name)));
    }
    println!("{}", found_product.as_ref().unwrap().name);
    Ok(found_product) 
}

async fn find_inventory_by_name(db: &DatabaseConnection, name: &str) -> Result<Option<inventory::Model>, DbErr> {
    let found_inventory: Option<inventory::Model> = Inventory::find()
    .filter(inventory::Column::Name.eq(name.to_owned()))
    .one(db)
    .await?;
    if let None = found_inventory {
        return Err(DbErr::Custom(format!("Inventory with name {} not found", name)));
    }
    println!("{}", found_inventory.as_ref().unwrap().name);
    Ok(found_inventory) 
}

async fn update_product(db: &DatabaseConnection, id: i32, name: &str, price: f64) -> Result<(), DbErr> {
    if price < 0.0 {
        return Err(DbErr::Custom("Price can't be negative.".to_owned()));
    }
    let find_product = find_product_by_id(db, id).await;
    if find_product.is_err() {
        return Err(DbErr::Custom(format!("Cannot update non-existing product")));
    }

    let updated_product = product::ActiveModel {
        id: ActiveValue::Set(id),
        name: ActiveValue::Set(name.to_owned()),
        price: ActiveValue::Set(price),
    };
    updated_product.update(db).await?;

    let inventory_id = fetch_inventory_by_product_id(db, id).await?;

    let updated_inventory = inventory::ActiveModel {
        id: ActiveValue::Set(inventory_id),
        name: ActiveValue::set(name.to_owned()),
        product_id: ActiveValue::set(id),
        ..Default::default()
    };
    updated_inventory.update(db).await?;
    Ok(())
}

async fn update_inventory_quantity(db: &DatabaseConnection, name: &str, quantity: i32) -> Result<(), DbErr> {
    let find_inventory = find_inventory_by_name(db, name).await;
    if find_inventory.is_err() {
        return Err(DbErr::Custom(format!("Cannot delete non-existing product in inventory")));
    }
    let inventory = find_inventory_by_name(db, name).await?.unwrap();
    let inventory_id = inventory.id;
    let capacity = inventory.capacity;

    if quantity < 0 {
        return Err(DbErr::Custom("Quantity can't be negative.".to_owned()));
    }
    if quantity > capacity {
        return Err(DbErr::Custom("Quantity can't exceed capacity.".to_owned()));
    }

    let stock = f64::from(quantity) / f64::from(capacity);
    let updated_inventory = inventory::ActiveModel {
        id: ActiveValue::Set(inventory_id),
        quantity: ActiveValue::Set(quantity),
        stock: ActiveValue::Set(stock), 
        ..Default::default()
    };
    updated_inventory.update(db).await?;
    Ok(())
}

async fn delete_product(db: &DatabaseConnection, id: i32) -> Result<(), DbErr> {
    let find_product = find_product_by_id(db, id).await;
    if find_product.is_err() {
        return Err(DbErr::Custom(format!("Cannot delete non-existing product")));
    }
    let deleted_product = product::ActiveModel {
        id: ActiveValue::Set(id),
        ..Default::default()
    };
    deleted_product.delete(db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{
        DatabaseBackend, MockDatabase,
    };

    // Test create product operation
    #[tokio::test]
    async fn test_create_product() -> Result<(), DbErr> {
        let db = &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([
                [product::Model {
                    id: 1,
                    name: "Test Product".to_owned(),
                    price: 10.0,
                }]
            ])
            .append_query_results([
                [inventory::Model {
                    id: 1,
                    name: "Test Product".to_owned(),
                    quantity: 100,
                    capacity: 100,
                    stock: 1.0,
                    product_id: 1,
                }],
            ])
            .append_exec_results([
                MockExecResult {
                    last_insert_id: 1,
                    rows_affected: 1,
                },
            ])
            .append_exec_results([
                MockExecResult {
                    last_insert_id: 1,
                    rows_affected: 1,
                },
            ])
            .into_connection();

        let result = create_product(db, "Test Product", 10.0, 100).await?;
        let (product_result, inventory_result) = result;
        assert_eq!(product_result, 
                product::Model {
                    id: 1,
                    name: "Test Product".to_owned(),
                    price: 10.0,
                }
        );
        assert_eq!(inventory_result, 
                inventory::Model {
                    id: 1,
                    name: "Test Product".to_owned(),
                    quantity: 100,
                    capacity: 100,
                    stock: 1.0,
                    product_id: 1,
                }
        );
        Ok(())
    }
    // Test error handling
    #[tokio::test]
    // Error: Capacity is zero
    async fn test_create_product_zero_capacity() {
        let db = &MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let result = create_product(db, "Test Product", 10.0, 0).await;
        let e = result.unwrap_err();
        assert_eq!(e, DbErr::Custom("Capacity can't be zero.".to_owned()));
    }
    #[tokio::test]
    // Error: Capacity is negative
    async fn test_create_product_negative_capacity() {
        let db = &MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let result = create_product(db, "Test Product", 10.0, -220).await;
        let e = result.unwrap_err();
        assert_eq!(e, DbErr::Custom("Capacity can't be negative.".to_owned()));
    }
    // Error: Price is negative
    async fn test_create_product_negative_price() {
        let db = &MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let result = create_product(db, "Test Product", -10.0, 100).await;
        let e = result.unwrap_err();
        assert_eq!(e, DbErr::Custom("Price can't be negative.".to_owned()));
    }
        
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
