/** @vitest-environment jsdom */
import { describe, expect, it } from "vitest";
import type { ForgeReview } from "@gitbutler/but-sdk";
import { coalesceInboxEntries } from "./review-notifications.ts";
import { addInboxEntries } from "./review-inbox.ts";

const review = (overrides: Partial<ForgeReview> = {}): ForgeReview => ({
	htmlUrl: "https://forge.example/pr/7",
	number: 7,
	title: "A change",
	body: null,
	author: null,
	labels: [],
	draft: false,
	sourceBranch: "feature",
	targetBranch: "main",
	sha: "abc",
	integrationCommitShas: [],
	createdAt: "2026-08-28T09:00:00Z",
	modifiedAt: "2026-08-28T10:00:00Z",
	mergedAt: null,
	closedAt: null,
	repositorySshUrl: null,
	repositoryHttpsUrl: null,
	repoOwner: null,
	headRepoIsFork: false,
	reviewers: [],
	autoMergeEnabled: false,
	unitSymbol: "#",
	lastSyncAt: "2026-08-28T10:00:00Z",
	...overrides,
});

describe("coalesceInboxEntries", () => {
	it("files human and agent comments from one poll separately, even at the same timestamp", () => {
		const atMs = Date.parse("2026-08-28T11:00:00Z");
		const items = coalesceInboxEntries(
			review(),
			[
				{ kind: "comment", id: 1, author: "alice", authorIsBot: false, body: "Human note", atMs },
				{ kind: "comment", id: 2, author: "copilot", authorIsBot: true, body: "Agent note", atMs },
				{
					kind: "comment",
					id: 3,
					author: "alice",
					authorIsBot: false,
					body: "Earlier human note",
					atMs: atMs - 1000,
				},
			],
			null,
		);
		expect(items).toHaveLength(2);
		expect(items[0]).toMatchObject({
			id: "7:comment:2026-08-28T11:00:00.000Z",
			author: "alice",
			authorIsBot: false,
			count: 2,
			commentId: 1,
			snippet: "Human note",
		});
		expect(items[1]).toMatchObject({
			id: "7:comment:2026-08-28T11:00:00.000Z:bot",
			author: "copilot",
			authorIsBot: true,
			count: 1,
			commentId: 2,
			snippet: "Agent note",
		});
		const projectId = "coalescing-test";
		expect(addInboxEntries(projectId, items)).toHaveLength(2);
		expect(
			JSON.parse(localStorage.getItem(`pr_activity_inbox:v1:${projectId}`) ?? "[]"),
		).toHaveLength(2);
	});
});
