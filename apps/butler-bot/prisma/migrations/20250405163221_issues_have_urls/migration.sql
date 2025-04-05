-- RedefineTables
PRAGMA defer_foreign_keys=ON;
PRAGMA foreign_keys=OFF;
CREATE TABLE "new_GitHubIssue" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "title" TEXT NOT NULL,
    "issue_number" INTEGER NOT NULL,
    "embedding" TEXT,
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "gitHubRepoId" INTEGER NOT NULL,
    "url" TEXT NOT NULL DEFAULT 'https://github.com',
    CONSTRAINT "GitHubIssue_gitHubRepoId_fkey" FOREIGN KEY ("gitHubRepoId") REFERENCES "GitHubRepo" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "new_GitHubIssue" ("created_at", "embedding", "gitHubRepoId", "id", "issue_number", "title", "updated_at") SELECT "created_at", "embedding", "gitHubRepoId", "id", "issue_number", "title", "updated_at" FROM "GitHubIssue";
DROP TABLE "GitHubIssue";
ALTER TABLE "new_GitHubIssue" RENAME TO "GitHubIssue";
CREATE INDEX "GitHubIssue_issue_number_idx" ON "GitHubIssue"("issue_number");
CREATE INDEX "GitHubIssue_gitHubRepoId_idx" ON "GitHubIssue"("gitHubRepoId");
PRAGMA foreign_keys=ON;
PRAGMA defer_foreign_keys=OFF;
