// A pull request on branch C with reviewers, checks and conversations, as the
// PR panel and its merge checklist see it.

import type {
	CiCheck,
	ForgeReviewComment,
	ForgeReviewSubmission,
	ForgeReviewThread,
} from "@gitbutler/but-sdk";
import { aReview, type FakeForge } from "./forge.ts";

const review = aReview({
	sourceBranch: "C",
	title: "Improve keyboard navigation in large repositories",
	body: "This change makes outstanding review work explicit.",
	labels: [
		{ name: "accessibility", color: "0e8a16", description: "Keyboard and screen reader support" },
		{ name: "@gitbutler/lite", color: "5319e7", description: null },
	],
	reviewers: [1, 2, 3, 4].map((id) => ({
		id,
		login: `reviewer${id}`,
		name: null,
		email: null,
		avatarUrl: null,
		isBot: false,
	})),
});
const author = {
	id: 5,
	login: "review-agent",
	name: null,
	email: null,
	avatarUrl: null,
	isBot: true,
};
const submissions: Array<ForgeReviewSubmission> = [
	{
		id: 10,
		author,
		state: "commented",
		body: "Please add coverage for pending reviews.",
		submittedAt: "2026-09-01T00:00:00Z",
		htmlUrl: review.htmlUrl,
		reactions: [],
	},
];
const comments: Array<ForgeReviewComment> = [
	{
		id: 20,
		author: { ...author, login: "alice", isBot: false },
		body: "## 🔵 Needs a closer look\n\nThe keyboard flow needs another pass.",
		createdAt: "2026-09-02T00:00:00Z",
		modifiedAt: null,
		htmlUrl: review.htmlUrl,
		reactions: [],
	},
	{
		id: 21,
		author,
		body: "## 🟡 Changes recommended\n\nPlease review the remaining edge cases.",
		createdAt: "2026-09-03T00:00:00Z",
		modifiedAt: null,
		htmlUrl: review.htmlUrl,
		reactions: [],
	},
];
// A file-level thread, so the feed has no line to check against the working file.
const threads: Array<ForgeReviewThread> = [
	{
		id: "thread-1",
		path: "src/navigation.ts",
		line: null,
		startLine: null,
		originalLine: null,
		side: "new",
		isResolved: false,
		isOutdated: false,
		comments: [
			{
				id: 30,
				author,
				body: "Arrow keys skip the last row.",
				createdAt: "2026-09-01T00:00:00Z",
				modifiedAt: null,
				htmlUrl: review.htmlUrl,
				diffHunk: null,
				reviewId: 10,
			},
		],
	},
];
const checks: Array<CiCheck> = [
	{
		id: 1,
		name: "Unit tests",
		output: { summary: "", text: "", title: "" },
		startedAt: "2026-09-01T00:00:00Z",
		status: { complete: { conclusion: "success", completed_at: "2026-09-01T00:01:00Z" } },
		headSha: review.sha,
		url: review.htmlUrl,
		htmlUrl: review.htmlUrl,
		detailsUrl: review.htmlUrl,
		pullRequests: [],
		reference: "C",
		lastSyncAt: "2026-09-01T00:01:00Z",
	},
];

/**
 * By default the review is blocked: four reviews pending and one unresolved
 * conversation. `clear` approves and resolves everything; `failing` fails the checks.
 */
export const mergeReadiness = ({
	clear = false,
	failing = false,
}: { clear?: boolean; failing?: boolean } = {}): FakeForge => ({
	reviews: [clear ? { ...review, reviewers: [] } : review],
	activity: {
		mergeStatus: {
			isMergeable: clear && !failing,
			mergeableState: clear && !failing ? "clean" : "blocked",
			commentsCount: 1,
		},
		checks: failing
			? checks.map((check) => ({
					...check,
					status: {
						complete: { conclusion: "failure" as const, completed_at: "2026-09-01T00:01:00Z" },
					},
				}))
			: checks,
		submissions: clear
			? submissions.map((submission) => ({ ...submission, state: "approved" as const }))
			: submissions,
		comments: clear ? [] : comments,
		threads: threads.map((thread) => ({ ...thread, isResolved: clear })),
	},
});
