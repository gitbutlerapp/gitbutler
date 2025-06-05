/*
  Warnings:

  - You are about to drop the `User` table. If the table is not empty, all the data it contains will be lost.

*/
-- DropTable
PRAGMA foreign_keys=off;
DROP TABLE "User";
PRAGMA foreign_keys=on;

-- CreateTable
CREATE TABLE "Butlers" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "discord_id" TEXT NOT NULL,
    "in_support_rota" BOOLEAN NOT NULL DEFAULT false
);
