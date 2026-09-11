import type { LiteElectronApi } from "#electron/ipc.ts";
import type {
	ForgeReview,
	ForgeInfo,
	ForgeReviewSubmission,
	ForgeReviewComment,
	CiCheck,
} from "@gitbutler/but-sdk";
import type { ElectronApplication, Page } from "@playwright/test";
export const review: ForgeReview = {
	number: 1,
	title: "Improve keyboard navigation in large repositories",
	sourceBranch: "C",
	targetBranch: "master",
	htmlUrl: "https://github.com/example/repo/pull/1",
	body: "This change makes pull request reviews easier to read.\n\nIt keeps the description visible and makes outstanding review work explicit.\n\nReviewers can scan the changed files alongside the conversation.\n\nThe keyboard shortcuts remain available throughout the review.",
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

export const openReadabilityReview = async (appWindow: Page, electronApp: ElectronApplication) => {
	await appWindow.setViewportSize({ width: 1440, height: 900 });
	const sample = await appWindow.evaluate(() =>
		(window as unknown as { lite: LiteElectronApi }).lite.branchDiff({
			projectId: location.pathname.split("/")[2] ?? "",
			branch: "refs/heads/C",
		}),
	);
	const first = sample.changes[0];
	if (!first) throw new Error("Expected a changed file");
	const changes = Array.from({ length: 12 }, (_, index) => ({
		...first,
		path: `src/file-${index}.ts`,
		pathBytes: Array.from(Buffer.from(`src/file-${index}.ts`)),
	}));

	await electronApp.evaluate(
		({ ipcMain }, data) => {
			const responses = {
				branchDiff: { ...data.sample, changes: data.changes },
				forgeInfo: data.forge,
				listReviews: [data.review],
				getReview: data.review,
				listKnownGithubAccounts: [{ type: "patUsername", info: { username: "test" } }],
				getReviewMergeStatus: { isMergeable: false, mergeableState: "blocked", commentsCount: 1 },
				listCiChecks: data.checks,
				listReviewSubmissions: data.submissions,
				listReviewComments: data.comments,
				listReviewThreads: [],
				listReviewTimelineEvents: [],
				listReviewReactions: [],
				currentForgeLogin: "test",
			};
			for (const [key, value] of Object.entries(responses)) {
				ipcMain.removeHandler(key);
				ipcMain.handle(key, () => value);
			}
		},
		{ review, forge, checks, submissions, comments, sample, changes },
	);
	await appWindow.reload();
	await appWindow
		.getByRole("treeitem", { name: "C", exact: true })
		.getByTitle("C", { exact: true })
		.click();
};
