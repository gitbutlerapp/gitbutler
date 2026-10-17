import type {
	CiCheck,
	ForgeInfo,
	ForgeReview,
	ForgeReviewComment,
	ForgeReviewSubmission,
	ForgeReviewThread,
	RefInfo,
} from "@gitbutler/but-sdk";
import type { App } from "./app.ts";

/** A forge the app talks to instead of GitHub. `reviews` are the pull requests it knows. */
export type FakeForge = {
	reviews: Array<ForgeReview>;
	/** What every review's page shows. Without it the forge has no checks or review comments. */
	activity?: ReviewActivity;
};

export type ReviewActivity = {
	mergeStatus: { isMergeable: boolean; mergeableState: string; commentsCount: number };
	checks: Array<CiCheck>;
	submissions: Array<ForgeReviewSubmission>;
	comments: Array<ForgeReviewComment>;
	threads: Array<ForgeReviewThread>;
};

export const github: ForgeInfo = {
	name: "github",
	baseUrl: "https://github.com/example/repo",
	commitUrlPath: "/commit/",
	prUrlPath: "/pull/",
	unit: { symbol: "#", name: "pull request", abbr: "PR" },
	posthogLabel: "",
	capabilities: {
		prService: true,
		checks: false,
		repoInfo: false,
		listService: false,
		reviewComments: false,
		reviewManagement: false,
	},
};

export const aReview = (overrides: Partial<ForgeReview> & Pick<ForgeReview, "sourceBranch">) =>
	({
		number: 1,
		title: `Update ${overrides.sourceBranch}`,
		targetBranch: "master",
		htmlUrl: `https://github.com/example/repo/pull/${overrides.number ?? 1}`,
		body: null,
		author: null,
		labels: [],
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
		reviewers: [],
		autoMergeEnabled: false,
		unitSymbol: "#",
		lastSyncAt: "2026-08-02T00:00:00Z",
		...overrides,
	}) satisfies ForgeReview;

const isOpen = (review: ForgeReview) => review.mergedAt === null && review.closedAt === null;

/**
 * The open-PR list omits merged and closed PRs, so the app finds those through the
 * review recorded on their branch, which is written into a frozen head info.
 */
export const installForge = async (app: App, { reviews, activity }: FakeForge): Promise<void> => {
	const finished = reviews.filter((review) => !isOpen(review));
	if (finished.length > 0) {
		const headInfo = await app.page.evaluate(
			(projectId) => window.lite.headInfo(projectId),
			app.projectId,
		);
		recordReviews(headInfo, finished);
		await app.backend.stub({ headInfo });
	}

	await app.backend.stub({
		forgeInfo: activity
			? { ...github, capabilities: { ...github.capabilities, checks: true, reviewComments: true } }
			: github,
		listReviews: reviews.filter(isOpen),
		listKnownGithubAccounts: [{ type: "patUsername", info: { username: "test" } }],
		...(activity && {
			getReviewMergeStatus: activity.mergeStatus,
			listCiChecks: activity.checks,
			listReviewSubmissions: activity.submissions,
			listReviewComments: activity.comments,
			listReviewThreads: activity.threads,
			listReviewTimelineEvents: [],
			listReviewReactions: [],
			currentForgeLogin: "test",
		}),
	});
	await app.electronApp.evaluate(({ ipcMain }, reviews) => {
		ipcMain.removeHandler("getReview");
		ipcMain.handle("getReview", (_event, { reviewId }: { reviewId: number }) =>
			reviews.find((review) => review.number === reviewId),
		);
	}, reviews);
};

const recordReviews = (headInfo: RefInfo, reviews: Array<ForgeReview>): void => {
	for (const review of reviews) {
		const branch = headInfo.stacks
			.flatMap((stack) => stack.segments)
			.find((segment) => segment.refName?.displayName === review.sourceBranch);
		if (!branch) throw new Error(`No branch ${review.sourceBranch} to record #${review.number} on`);
		branch.metadata = {
			refInfo: { createdAt: null, updatedAt: null },
			review: { pullRequest: review.number, reviewId: null },
		};
	}
};
