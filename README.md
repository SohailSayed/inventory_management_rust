# Inventory Management System Rust

Basic inventory management system implemented with Rust, Postgres and Docker.



## To Run

To run:
```
docker compose up --build
```

**Note:** This both runs the code, and unit tests

## Description

The core functionality of this application lies in `src/main.rs`

## Design Decisions
- Magic numbers avoided as much as possible;
- Unit tests split into separate functions for each test case;
- Each function has a single responsibility;
- Database is wiped/recreated every run to ensure clean run;
- Error handling include custom messages, to make debugging easier;
- Mock database used for unit tests, to allow testing without spinning up and relying on external database service;
- All core functions are written in one file - main.rs. This is to simplify development and allow easy access to all functions, as the scope of this project is relatively small.

## Assumptions
- One warehouse; one-to-one relation between Product and Inventory - changes in Product reflected in Inventory;
- Unique names for each product;
- Assumed very large numbers would not be involved;
- Low stock is defined as being at 30% of total capacity or lower;
- Prices are static;

## Trade-Offs
- Simplicity for scalability - keeping all core functionalities in main.rs has made development simple at the cost of being scalable, since the app isn't expected to scale beyond how it currently is;
- Dockerfile can be optimized further, but is retained as is for it's simplicity in use during development;
- Clean database for persistence - data is wiped every run, preventing persistance, however this allows consistent runs during testing and development;
- Duplication between functions - there's some duplication between functions, however readibility and practical use are prioritized over performance here;
  
## Use of AI tools
AI tools have been used extensively for learning and debugging during development, but sparingly to write actual code;



