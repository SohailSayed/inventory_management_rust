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

struct StoreProduct {
    name: String,
    price: f64,
    capacity: i32,
}

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

    let sample_product_2 = StoreProduct {
        name: "Sample Product 2".to_owned(),
        price: 20.0,
        capacity: 100,
    };
    // Insert a product called "Sample Product 2"
    create_product(db, &sample_product_2.name, sample_product_2.price, sample_product_2.capacity).await?;

    // Find a product with the ID 1
    find_product_by_id(db, 1).await?;

    // Find a product with the name "Sample Product 2"
    find_product_by_name(db, "Sample Product 2").await?;

    let id_to_update = 1;
    let updated_name = "Updated Product Name".to_owned();
    let updated_price = 30.0;
    // Update information of product with ID 1
    update_product(db, id_to_update, &updated_name, updated_price).await?;

    // Delete product with ID 1
    delete_product(db, 1).await?;

    let sample_product_3 = StoreProduct {
        name: "Sample Product 3".to_owned(),
        price: 55.0,
        capacity: 300,
    };
    // Insert a product called "Sample Product 3"
    create_product(db, &sample_product_3.name, sample_product_3.price, sample_product_3.capacity).await?;
    // Update the quantity of "Sample Product 3" to 151
    update_inventory_quantity(db, "Sample Product 3", 151).await?; 

    let sample_product_4 = StoreProduct {
        name: "Sample Product 4".to_owned(),
        price: 55.0,
        capacity: 20,
    };
    // Insert a product called "Sample Product 4"
    create_product(db, &sample_product_4.name, sample_product_4.price, sample_product_4.capacity).await?;
    // Update the quantity of "Sample Product 4" to 1
    update_inventory_quantity(db, "Sample Product 4", 1).await?; 

    let sample_product_5 = StoreProduct {
        name: "Sample Product 5".to_owned(),
        price: 55.0,
        capacity: 200,
    };
    // Insert a product called "Sample Product 5"
    create_product(db, &sample_product_5.name, sample_product_5.price, sample_product_5.capacity).await?;
    // Update the quantity of "Sample Product 5" to 3
    update_inventory_quantity(db, "Sample Product 5", 3).await?; 

    // Retrieve products low in stock
    retrieve_low_stock(db, LOW_THRESHOLD).await?;
    // Caculate the total inventory valueß
    calculate_total_inventory_value(db).await?;

    Ok(())
}

async fn calculate_total_inventory_value(db: &DatabaseConnection) -> Result<f64, DbErr> {
    // Calculate total dollar value of inventory
    let inventory: Vec<inventory::Model> = Inventory::find().all(db).await?;
    let mut total_value: f64 = 0.0;
    for product in &inventory {
        let product_id = product.product_id;
        let quantity = product.quantity;
        let price = find_product_by_id(db, product_id).await?.price;
        let product_value = f64::from(quantity) * price;
        total_value += product_value;
    }
    println!("Total inventory value: ${}", total_value);
    Ok(total_value)
}

async fn retrieve_low_stock(db: &DatabaseConnection, threshold: f64) -> Result<Vec<inventory::Model>, DbErr> {
    // Retrieve all products that are stocked less than 30% their total capacity
    let max_threshold = 1.00;
    if threshold > max_threshold {
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
    // Insert a product into product and inventory tables
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

    // One to one relationship - changes in product reflected in inventory
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

async fn find_product_by_id(db: &DatabaseConnection, id: i32) -> Result<product::Model, DbErr> {
    // Find a product by ID
    println!("{}", id);
    let found_product: Option<product::Model> = Product::find_by_id(id).one(db).await?;
    if let None = found_product {
        return Err(DbErr::Custom("Product with this ID not found.".to_owned()));
    }
    
    println!("{}", found_product.as_ref().unwrap().name.to_owned());
    Ok(product::Model {
        id: id,
        name: found_product.as_ref().unwrap().name.to_owned(),
        price: found_product.as_ref().unwrap().price,
    }) 
}  

async fn find_product_by_name(db: &DatabaseConnection, name: &str) -> Result<product::Model, DbErr> {
    // Find a product by name (unique)
    let found_product: Option<product::Model> = Product::find()
    .filter(product::Column::Name.eq(name.to_owned()))
    .one(db)
    .await?;
    if let None = found_product {
        return Err(DbErr::Custom("Product with this name not found.".to_owned()));
    }
    println!("Product found: {}", found_product.as_ref().unwrap().name);
    Ok(product::Model {
        id: found_product.as_ref().unwrap().id,
        name: name.to_owned(),
        price: found_product.as_ref().unwrap().price,
    }) 
}

async fn fetch_inventory_by_product_id(db: &DatabaseConnection, product_id: i32) -> Result<i32, DbErr> {
    // Fetch inventory ID by corresponding product ID
    let fetched_inventory: Option<inventory::Model> = Inventory::find()
    .filter(inventory::Column::ProductId.eq(product_id))
    .one(db)
    .await?;
    if let None = fetched_inventory {
        return Err(DbErr::Custom("Inventory with this Product ID not found".to_owned()));
    }
    println!("Inventory fetched: {}", fetched_inventory.as_ref().unwrap().id);
    Ok(fetched_inventory.as_ref().unwrap().id)
}

async fn find_inventory_by_name(db: &DatabaseConnection, name: &str) -> Result<inventory::Model, DbErr> {
    // Find inventory by product name
    let found_inventory: Option<inventory::Model> = Inventory::find()
    .filter(inventory::Column::Name.eq(name.to_owned()))
    .one(db)
    .await?;
    if let None = found_inventory {
        return Err(DbErr::Custom("Inventory with this name not found.".to_owned()));
    }
    println!("Inventory found: {}", found_inventory.as_ref().unwrap().name);
    Ok(inventory::Model {
        id: found_inventory.as_ref().unwrap().id,
        name: name.to_owned(),
        quantity: found_inventory.as_ref().unwrap().quantity,
        capacity: found_inventory.as_ref().unwrap().capacity,
        stock: found_inventory.as_ref().unwrap().stock,
        product_id: found_inventory.as_ref().unwrap().product_id,
    }) 
}

async fn update_product(db: &DatabaseConnection, id: i32, name: &str, price: f64) -> Result<(product::Model, inventory::Model), DbErr> {
    // Update product information, reflect changes in inventory
    if price < 0.0 {
        return Err(DbErr::Custom("Price can't be negative.".to_owned()));
    }

    let find_product = find_product_by_id(db, id).await;
    if find_product.is_err() {
        return Err(DbErr::Custom("Cannot update non-existing product.".to_owned()));
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

    let returned_inventory = find_inventory_by_name(db, name).await?;
    Ok((
        product::Model {
            id: id,
            name: name.to_owned(),
            price: price,
        },
        inventory::Model {
            id: inventory_id,
            name: name.to_owned(),
            quantity: returned_inventory.quantity,
            capacity: returned_inventory.capacity,
            stock: returned_inventory.stock,
            product_id: id,
        }
    ))
}

async fn update_inventory_quantity(db: &DatabaseConnection, name: &str, new_quantity: i32) -> Result<inventory::Model, DbErr> {
    // Update inventory product quantity
    let find_inventory = find_inventory_by_name(db, name).await;
    let inventory = find_inventory_by_name(db, name).await?;
    let inventory_id = inventory.id;
    let capacity = inventory.capacity;

    if find_inventory.is_err() {
        return Err(DbErr::Custom(format!("Cannot delete non-existing product in inventory.")));
    }
    else if new_quantity < 0 {
        return Err(DbErr::Custom("Quantity can't be negative.".to_owned()));
    }
    else if new_quantity > capacity {
        return Err(DbErr::Custom("Quantity can't exceed capacity.".to_owned()));
    }

    let stock = f64::from(new_quantity) / f64::from(capacity);
    let updated_inventory = inventory::ActiveModel {
        id: ActiveValue::Set(inventory_id),
        quantity: ActiveValue::Set(new_quantity),
        stock: ActiveValue::Set(stock), 
        ..Default::default()
    };
    updated_inventory.update(db).await?;

    let returned_inventory = find_inventory_by_name(db, name).await?;
    Ok(inventory::Model {
        id: returned_inventory.id,
        name: name.to_owned(),
        quantity: new_quantity,
        capacity: returned_inventory.capacity,
        stock: returned_inventory.stock,
        product_id: returned_inventory.product_id,
    })
}

async fn delete_product(db: &DatabaseConnection, id: i32) -> Result<(), DbErr> {
    // Delete product, reflect deletion in inventory
    let find_product = find_product_by_id(db, id).await;
    if find_product.is_err() {
        return Err(DbErr::Custom("Cannot delete non-existing product.".to_owned()));
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
    // Unit Tests:
    use super::*;
    use sea_orm::{
        DatabaseBackend, MockDatabase,
    };

    mod create_product_tests {
        use super::*;

        // 1. Test create_product operation
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
        // create_product error handling tests
        #[tokio::test]
        // Error: Capacity is zero
        async fn test_create_product_zero_capacity() {
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres).into_connection();
            let result = create_product(empty_db, "Test Product", 10.0, 0).await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Capacity can't be zero.".to_owned()));
        }
        #[tokio::test]
        // Error: Capacity is negative
        async fn test_create_product_negative_capacity() {
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres).into_connection();
            let result = create_product(empty_db, "Test Product", 10.0, -220).await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Capacity can't be negative.".to_owned()));
        }
        #[tokio::test]
        // Error: Price is negative
        async fn test_create_product_negative_price() {
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres).into_connection();
            let result = create_product(empty_db, "Test Product", -10.0, 100).await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Price can't be negative.".to_owned()));
        }
    }
    
    mod find_product_by_id_tests {
        use super::*;

        // 2. Test find_product_by_id operation
        #[tokio::test]
        async fn test_find_product_by_id() {
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

            let result = find_product_by_id(db, 1).await;

            assert_eq!(result, 
                Ok(product::Model {
                    id: 1,
                    name: "Test Product".to_owned(),
                    price: 10.0,
                })
            );
        }
        // find_product_by_id error handling tests
        // Error: product not found
        #[tokio::test]
        async fn test_find_product_by_id_invalid() {
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<product::Model>::new()])
            .into_connection();

            let result = find_product_by_id(empty_db, 30).await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Product with this ID not found.".to_owned()));
        }
    }
    
    mod find_product_by_name_tests {
        use super::*;

        // 3. Test find_product_by_name operation
        #[tokio::test]
        async fn test_find_product_by_name() {
            let db = &MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([
                    [product::Model {
                        id: 1,
                        name: "Test Product".to_owned(),
                        price: 10.0,
                    }]
                ])
                .into_connection();

            let result = find_product_by_name(db, "Test Product").await;

            assert_eq!(result, 
                Ok(product::Model {
                    id: 1,
                    name: "Test Product".to_owned(),
                    price: 10.0,
                })
            );
        }
        // find_product_by_name error handling tests
        // Error: product not found
        #[tokio::test]
        async fn test_find_product_by_name_invalid() {
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<product::Model>::new()])
            .into_connection();

            let result = find_product_by_name(empty_db, "Invalid Name").await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Product with this name not found.".to_owned()));
        }
    }

    mod fetch_inventory_by_product_id_tests {
        use super::*;

        // 4. Test fetch_inventory_by_product_id operation
        #[tokio::test]
        async fn test_fetch_inventory_by_product_id(){
            let db = &MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([
                    [inventory::Model {
                        id: 2,
                        name: "Test Product".to_owned(),
                        quantity: 100,
                        capacity: 100,
                        stock: 1.0,
                        product_id: 1,
                    }]
                ])
                .append_query_results([
                    [product::Model {
                        id: 1,
                        name: "Test Product".to_owned(),
                        price: 10.0,
                    }]
                ])
                .into_connection();

            let result = fetch_inventory_by_product_id(db, 1).await;
            let correct_inventory_id = 2;
            assert_eq!(result, Ok(correct_inventory_id));
        }
        // fetch_inventory_by_product_id error handling tests
        // Error: inventory not found
        #[tokio::test]
        async fn test_fetch_inventory_by_product_id_invalid(){
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<product::Model>::new()])
            .into_connection();

            let result = fetch_inventory_by_product_id(empty_db, 1).await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Inventory with this Product ID not found".to_owned()));
        }
    }

    mod find_inventory_by_name_tests {
        use super::*;

        // 5. Test find_inventory_by_name operation
        #[tokio::test]
        async fn test_find_inventory_by_name(){
            let db = &MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([
                    [inventory::Model {
                        id: 2,
                        name: "Test Product".to_owned(),
                        quantity: 100,
                        capacity: 100,
                        stock: 1.0,
                        product_id: 1,
                    }]
                ])
                .into_connection();

            let result = find_inventory_by_name(db, "Test Product").await;
            assert_eq!(result,
                Ok(inventory::Model {
                    id: 2,
                    name: "Test Product".to_owned(),
                    quantity: 100,
                    capacity: 100,
                    stock: 1.0,
                    product_id: 1,
                })
            );
        }
        // find_inventory_by_name error handling tests
        // Error: inventory not found
        #[tokio::test]
        async fn test_find_inventory_by_name_invalid(){
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<product::Model>::new()])
            .into_connection();

            let result = find_inventory_by_name(empty_db, "Invalid Product").await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Inventory with this name not found.".to_owned()));
        }
    }

    mod update_product_tests {
        use super::*;

        // 6. Test update_product operation
        // #[tokio::test]
        // async fn test_update_product() {
        //     let db = &MockDatabase::new(DatabaseBackend::Postgres)
        //         .append_exec_results([
        //             MockExecResult {
        //                 last_insert_id: 1,
        //                 rows_affected: 1,
        //             },
        //         ])
        //         .append_exec_results([
        //             MockExecResult {
        //                 last_insert_id: 1,
        //                 rows_affected: 1,
        //             },
        //         ])
        //         .append_query_results([
        //             [product::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 price: 10.0,
        //             }]
        //         ])
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }],
        //         ])
        //         .into_connection();
            
        //     let result = update_product(db, 1, "Updated Test Product", 20.0).await;
        //     let (product_result, inventory_result) = result.unwrap();
        //     assert_eq!(product_result, 
        //             product::Model {
        //                 id: 1,
        //                 name: "Updated Test Product".to_owned(),
        //                 price: 20.0,
        //             }
        //     );
        //     assert_eq!(inventory_result, 
        //             inventory::Model {
        //                 id: 1,
        //                 name: "Updated Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }
        //     );
        // }
        // // update_product error handling tests
        // // Error: product not found
        // #[tokio::test]
        // async fn test_update_product_invalid(){
        //     let empty_db = &MockDatabase::new(DatabaseBackend::Postgres)
        //     .append_query_results([Vec::<product::Model>::new()])
        //     .into_connection();

        //     let result = update_product(empty_db, 1, "Updated Test Product", 20.0).await;
        //     let e = result.unwrap_err();
        //     assert_eq!(e, DbErr::Custom("Cannot update non-existing product.".to_owned()));
        // }
        // // Error: negative price
        // #[tokio::test]
        // async fn test_update_product_negative_price(){
        //     let empty_db = &MockDatabase::new(DatabaseBackend::Postgres)
        //     .append_query_results([Vec::<product::Model>::new()])
        //     .into_connection();

        //     let result = update_product(empty_db, 1, "Updated Test Product", -20.0).await;
        //     let e = result.unwrap_err();
        //     assert_eq!(e, DbErr::Custom("Price can't be negative.".to_owned()));
        // }
    }

    mod update_inventory_quantity_tests {
        use super::*;

        // // 7. Test update_inventory_quantity operation
        // #[tokio::test]
        // async fn test_update_inventory_quantity() {
        //     let db = &MockDatabase::new(DatabaseBackend::Postgres)
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }],
        //         ])
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }],
        //         ])
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 50,
        //                 capacity: 100,
        //                 stock: 0.5,
        //                 product_id: 1,
        //             }],
        //         ])
        //         .into_connection();
            
        //     let result = update_inventory_quantity(db, "Test Product", 50).await;
        //     assert_eq!(result, 
        //             Ok(inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 50,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             })
        //     );
        // }
        // // update_inventory_quantity error handling tests
        // // Error: product not found
        // #[tokio::test]
        // async fn test_update_inventory_quantity_invalid(){
        //     let db = &MockDatabase::new(DatabaseBackend::Postgres)
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }],
        //         ])
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }],
        //         ])
        //         .into_connection();

        //     let result = update_inventory_quantity(db, "Invalid Product", 50).await;
        //     let e = result.unwrap_err();
        //     assert_eq!(e, DbErr::Custom("Cannot delete non-existing product in inventory.".to_owned()));
        // }
        // // Error: negative quantity
        // #[tokio::test]
        // async fn test_update_inventory_quantity_negative_quantity(){
        //     let db = &MockDatabase::new(DatabaseBackend::Postgres)
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }],
        //         ])
        //     .into_connection();

        //     let result = update_inventory_quantity(db, "Test Product", -50).await;
        //     let e = result.unwrap_err();
        //     assert_eq!(e, DbErr::Custom("Quantity can't be negative.".to_owned()));
        // }
        // // Error: quantity greater than capacity
        // #[tokio::test]
        // async fn test_update_inventory_quantity_invalid_quantity(){
        //     let db = &MockDatabase::new(DatabaseBackend::Postgres)
        //         .append_query_results([
        //             [inventory::Model {
        //                 id: 1,
        //                 name: "Test Product".to_owned(),
        //                 quantity: 100,
        //                 capacity: 100,
        //                 stock: 1.0,
        //                 product_id: 1,
        //             }],
        //         ])
        //     .into_connection();

        //     let result = update_inventory_quantity(db, "Test Product", 200).await;
        //     let e = result.unwrap_err();
        //     assert_eq!(e, DbErr::Custom("Quantity can't exceed capacity.".to_owned()));
        // }
    }

    mod delete_product_tests {
        use super::*;

        // 8. Test delete_product operation
        #[tokio::test]
        async fn test_delete_product() {
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

            let result = delete_product(db, 1).await;
            assert!(result.is_ok()); 
        }
        // update_inventory_quantity error handling tests
        // Error: product not found
        #[tokio::test]
        async fn test_delete_product_invalid() {
            let empty_db = &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<product::Model>::new()])
            .into_connection();

            let result = delete_product(empty_db, 1).await;
            let e = result.unwrap_err();
            assert_eq!(e, DbErr::Custom("Cannot delete non-existing product.".to_owned()));
        }
    }

    // I was unable to make unit tests for retrieve_low_stock, due to time constraints
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
