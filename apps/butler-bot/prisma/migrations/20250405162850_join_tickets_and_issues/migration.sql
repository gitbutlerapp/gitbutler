-- CreateTable
CREATE TABLE "_GitHubIssueToSupportTicket" (
    "A" INTEGER NOT NULL,
    "B" INTEGER NOT NULL,
    CONSTRAINT "_GitHubIssueToSupportTicket_A_fkey" FOREIGN KEY ("A") REFERENCES "GitHubIssue" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "_GitHubIssueToSupportTicket_B_fkey" FOREIGN KEY ("B") REFERENCES "SupportTicket" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "_GitHubIssueToSupportTicket_AB_unique" ON "_GitHubIssueToSupportTicket"("A", "B");

-- CreateIndex
CREATE INDEX "_GitHubIssueToSupportTicket_B_index" ON "_GitHubIssueToSupportTicket"("B");
