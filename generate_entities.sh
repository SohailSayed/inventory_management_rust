# Much like the rest of this application, values, like the port and password here, are being hardcoded. This doesn't reflect how production code would look
sea-orm-cli generate entity \
    -u postgres://postgres:password123@db:54320/warehouse_db \
    -o src/entities
