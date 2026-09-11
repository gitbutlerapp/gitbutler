import type {
	ForgeReview,
	ForgeInfo,
	ForgeReviewSubmission,
	ForgeReviewComment,
	ForgeReviewThread,
	CiCheck,
} from "@gitbutler/but-sdk";
import type { ElectronApplication, Page } from "@playwright/test";
const review: ForgeReview = {
	number: 1,
	title: "Improve keyboard navigation in large repositories",
	sourceBranch: "C",
	targetBranch: "master",
	htmlUrl: "https://github.com/example/repo/pull/1",
	body: "This change makes outstanding review work explicit.",
	author: null,
	labels: [
		{ name: "accessibility", color: "0e8a16", description: "Keyboard and screen reader support" },
		{ name: "@gitbutler/lite", color: "5319e7", description: null },
	],
	draft: false,
	sha: "0000000000000000000000000000000000000001",
	integrationCommitShas: [],
	createdAt: "2026-08-01T00:00:00Z",
	modifiedAt: "2026-08-02T00:00:00Z",
	mergedAt: null,
	closedAt: null,
	repositorySshUrl: null,
	repositoryHttpsUrl: null,
	repoOwner: null,
	headRepoIsFork: false,
	reviewers: [1, 2, 3, 4].map((id) => ({
		id,
		login: `reviewer${id}`,
		name: null,
		email: null,
		avatarUrl: null,
		isBot: false,
	})),
	autoMergeEnabled: false,
	unitSymbol: "#",
	lastSyncAt: "2026-08-02T00:00:00Z",
};
const forge: ForgeInfo = {
	name: "github",
	baseUrl: "https://github.com/example/repo",
	commitUrlPath: "/commit/",
	prUrlPath: "/pull/",
	unit: { symbol: "#", name: "pull request", abbr: "PR" },
	posthogLabel: "",
	capabilities: {
		prService: true,
		checks: true,
		repoInfo: false,
		listService: false,
		reviewComments: true,
		reviewManagement: false,
	},
};
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

export type ReviewState = {
	/** Everyone has approved, every conversation is resolved and nothing is pending. */
	clear?: boolean;
	failing?: boolean;
	review?: Partial<ForgeReview>;
};

export const setChecklistReviewState = async (
	electronApp: ElectronApplication,
	options: ReviewState = {},
) => {
	const currentReview = {
		...review,
		...(options.clear ? { reviewers: [] } : {}),
		...options.review,
	};
	const currentChecks = options.failing
		? checks.map((check) => ({
				...check,
				status: {
					complete: { conclusion: "failure" as const, completed_at: "2026-09-01T00:01:00Z" },
				},
			}))
		: checks;
	await electronApp.evaluate(
		({ ipcMain }, data) => {
			const responses = {
				forgeInfo: data.forge,
				listReviews: [data.review],
				getReview: data.review,
				listKnownGithubAccounts: [{ type: "patUsername", info: { username: "test" } }],
				getReviewMergeStatus: data.mergeStatus,
				listCiChecks: data.checks,
				listReviewSubmissions: data.submissions,
				listReviewComments: data.comments,
				listReviewThreads: data.threads,
				listReviewTimelineEvents: [],
				listReviewReactions: [],
				currentForgeLogin: "test",
			};
			for (const [key, value] of Object.entries(responses)) {
				ipcMain.removeHandler(key);
				ipcMain.handle(key, () => value);
			}
		},
		{
			review: currentReview,
			forge,
			checks: currentChecks,
			submissions: options.clear
				? submissions.map((submission) => ({ ...submission, state: "approved" as const }))
				: submissions,
			comments: options.clear ? [] : comments,
			threads: threads.map((thread) => ({ ...thread, isResolved: options.clear === true })),
			mergeStatus: {
				isMergeable: options.clear === true && !options.failing,
				mergeableState: options.clear && !options.failing ? "clean" : "blocked",
				commentsCount: 1,
			},
		},
	);
};

export const openMergeReadinessReview = async (
	appWindow: Page,
	electronApp: ElectronApplication,
	state: ReviewState = {},
) => {
	await appWindow.setViewportSize({ width: 1440, height: 900 });
	await setChecklistReviewState(electronApp, state);
	await appWindow.reload();
	await appWindow
		.getByRole("treeitem", { name: "C", exact: true })
		.getByTitle("C", { exact: true })
		.click();
};
