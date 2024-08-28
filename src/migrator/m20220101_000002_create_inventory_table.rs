use sea_orm_migration::prelude::*;

use super::m20220101_000001_create_product_table::Product;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m_20220101_000002_create_inventory_table" 
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Inventory::Table)
                    .col(
                        ColumnDef::new(Inventory::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Inventory::Name).string().not_null())
                    .col(ColumnDef::new(Inventory::Quantity).integer().not_null())
                    .col(ColumnDef::new(Inventory::ProductId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-inventory-product_id")
                            .from(Inventory::Table, Inventory::ProductId)
                            .to(Product::Table, Product::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Inventory::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Inventory{
    Table,
    Id,
    Name,
    Quantity,
    ProductId,
}
