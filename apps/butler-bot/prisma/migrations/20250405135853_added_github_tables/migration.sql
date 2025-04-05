-- CreateTable
CREATE TABLE "GitHubIssue" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "title" TEXT NOT NULL,
    "issue_number" INTEGER NOT NULL,
    "embedding" TEXT NOT NULL,
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "gitHubRepoId" INTEGER NOT NULL,
    CONSTRAINT "GitHubIssue_gitHubRepoId_fkey" FOREIGN KEY ("gitHubRepoId") REFERENCES "GitHubRepo" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "GitHubRepo" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "name" TEXT NOT NULL,
    "owner" TEXT NOT NULL,
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateIndex
CREATE INDEX "GitHubIssue_issue_number_idx" ON "GitHubIssue"("issue_number");

-- CreateIndex
CREATE INDEX "GitHubIssue_gitHubRepoId_idx" ON "GitHubIssue"("gitHubRepoId");
