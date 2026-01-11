# Database Migration Policy

**Baseline Version: 0.2.5**

All database migrations from this point forward MUST maintain backwards compatibility with the v0.2.5 schema.

## Rules for Future Migrations

### ✅ ALLOWED
- **Add new tables** - Use `CREATE TABLE IF NOT EXISTS`
- **Add new columns** - Must be `NULL`able OR have a `DEFAULT` value
- **Add new indexes** - Use `CREATE INDEX IF NOT EXISTS`
- **Insert new rows** - Use `INSERT OR IGNORE` / `INSERT OR REPLACE`

### ❌ FORBIDDEN
- **Never remove columns** - Old data must remain accessible
- **Never rename columns** - Create new column + migrate data if needed
- **Never change column types** - Add a new column instead
- **Never remove tables** - Mark as deprecated in comments instead
- **Never add NOT NULL columns without defaults**
- **Never modify existing migration files** - Once a migration is applied, it is immutable. Changing it breaks database integrity checksums. Creates a NEW migration file for any changes.

## Schema Version Tracking

The `schema_info` table tracks:
- `schema_version` - Integer version of the schema (increments with each migration)
- `min_compatible_version` - Minimum app version that can read this DB

## Migration Naming Convention

```
YYYYMMDDHHMMSS_description.sql
```

Example: `20260109210000_add_notifications_table.sql`

## Testing Migrations

Before releasing any migration:
1. Test upgrading from v0.2.5 database
2. Verify all existing queries still work
3. Verify new features work with fresh DB
4. Verify new features gracefully handle missing data in old DBs
