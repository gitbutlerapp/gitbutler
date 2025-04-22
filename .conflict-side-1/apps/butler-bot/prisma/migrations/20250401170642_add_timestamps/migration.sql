-- RedefineTables
PRAGMA defer_foreign_keys=ON;
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_Butlers" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "discord_id" TEXT NOT NULL,
    "name" TEXT NOT NULL DEFAULT '',
    "in_support_rota" BOOLEAN NOT NULL DEFAULT false,
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "new_Butlers" ("discord_id", "id", "in_support_rota", "name") SELECT "discord_id", "id", "in_support_rota", "name" FROM "Butlers";
DROP TABLE "Butlers";
ALTER TABLE "new_Butlers" RENAME TO "Butlers";
PRAGMA foreign_keys=ON;
PRAGMA defer_foreign_keys=OFF;
