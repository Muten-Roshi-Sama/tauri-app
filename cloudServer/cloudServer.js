// server.js
import express from "express";
import Database from "better-sqlite3";

const app = express();
app.use(express.json());

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => console.log(` Server running on port ${PORT}`));


//*____________ Setup SQLite (stores data in a file "licenses.db")____________
const db = new Database("licenses.db");

// Create table if not exists
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


// Endpoint: validate license
app.post("/validate", (req, res) => {
  const { key } = req.body;
  
  // Input validation
  if (!key) {
    return res.status(200).json({ 
      success: false, 
      message: "❌ License key is required" 
    });
  }

  try {
    const stmt = db.prepare("SELECT valid, expiry FROM licenses WHERE key = ?");
    const row = stmt.get(key);

    if (row && row.valid === 1) {
      // Check if license has expired
      const currentDate = new Date().toISOString().split('T')[0];
      if (row.expiry && row.expiry < currentDate) {
        return res.status(200).json({ 
          success: false, 
          message: "❌ License has expired" 
        });
      }
      
      res.status(200).json({ 
        success: true, 
        message: "✅ License valid" 
      });
    } else {
      res.status(200).json({ 
        success: false, 
        message: "❌ License invalid or not found" 
      });
    }
  } catch (error) {
    console.error("Database error:", error);
    res.status(200).json({ 
      success: false, 
      message: "❌ Server error during validation" 
    });
  }
});

// Endpoint: ping (for health checks)
app.get("/ping", (req, res) => {
  res.json({ message: "Server alive ✅" });
});




//_____ DEPENDENCIES______
// npm init -y
// npm install express
// npm install better-sqlite3

// npm install --save-dev nodemon

