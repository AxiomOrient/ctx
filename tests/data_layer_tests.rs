use ctxset::Result;
use ctxset::data::index::sqlite::SqliteIndexManager;

/// A helper to create an in-memory SQLite database for testing.
fn setup_in_memory_db() -> Result<SqliteIndexManager> {
    // ":memory:" tells SQLite to create a temporary in-memory database.
    SqliteIndexManager::new(":memory:")
}

#[test]
fn test_in_memory_db_creation_and_schema() -> Result<()> {
    // 1. Setup
    // Attempt to create an in-memory database and apply the schema.
    let manager = setup_in_memory_db()?;

    // 3. Verify schema creation by checking for a known table.
    let mut stmt = manager
        .get_connection()
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='docs'")?;
    let table_exists: bool = stmt.exists([])?;

    assert!(
        table_exists,
        "The 'docs' table should exist after schema creation."
    );

    Ok(())
}
