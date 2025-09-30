import Database from "better-sqlite3";

const db = new Database("licenses.db");

// Create table if missing
db.prepare(`
    CREATE TABLE IF NOT EXISTS licenses (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        key TEXT UNIQUE,
        email TEXT,
        first_name TEXT,
        last_name TEXT,
        valid INTEGER DEFAULT 1,
        expiry DATE
    )
`).run();

// Add a license key
function addLicense(key, email, firstName, lastName, expiry = null) {
    db.prepare(`
        INSERT OR REPLACE INTO licenses (key, email, first_name, last_name, valid, expiry) 
        VALUES (?, ?, ?, ?, ?, ?)
    `).run(key, email, firstName, lastName, 1, expiry);

    console.log(`✅ License ${key} added for ${firstName} ${lastName} (${email})`);
}



// Remove license: exactly ONE argument must be provided (id OR email OR key)
function removeLicense({ id = null, email = null, key = null } = {}) {
    /// Remove single license by Index
    ///  OR Remove ALL associated licenses by email
    ///  OR Remove single license by key

    // Count how many arguments are provided
    const argsProvided = [id, email, key].filter((arg) => arg !== null).length;

    if (argsProvided === 0) {
        console.log("⚠️ Please provide either id, email, or key to remove a license");
        return;
    }
    if (argsProvided > 1) {
        console.log("⚠️ Only one argument allowed (choose either id, email, OR key)");
        return;
    }

    if (id !== null) {
        const info = db.prepare("DELETE FROM licenses WHERE id = ?").run(id);
        if (info.changes > 0) {
            console.log(`🗑️ License with id ${id} removed`);
        } else {
            console.log(`⚠️ No license found with id ${id}`);
        }
    } else if (email !== null) {
        const info = db.prepare("DELETE FROM licenses WHERE email = ?").run(email);
        if (info.changes > 0) {
            console.log(`🗑️ All licenses for email ${email} removed`);
        } else {
            console.log(`⚠️ No licenses found for email ${email}`);
        }
    } else if (key !== null) {
        const info = db.prepare("DELETE FROM licenses WHERE key = ?").run(key);
        if (info.changes > 0) {
            console.log(`🗑️ License with key ${key} removed`);
        } else {
            console.log(`⚠️ No license found with key ${key}`);
        }
    }
}


// Print all licenses
function showDB() {
  const rows = db.prepare("SELECT * FROM licenses").all();
    if (rows.length === 0) {
        console.log("⚠️  No licenses in database yet.");
        return;
    }
    console.log("\n📋 Current Licenses:");
    console.table(rows);
}




// Example
addLicense("TEST-123", "user@example.com", "John", "Doe", "2025-12-31");
// removeLicense({ id: 23 });                 // Highest priority → removes row with id=1
// removeLicense({ email: "jane@example.com" }); // Removes all by email
// removeLicense({ key: "TEST-123" });          // Removes by key
showDB();