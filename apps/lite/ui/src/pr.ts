import type { AggregateCIStatus } from "./ci.ts";
import { loginKey } from "./review-users.ts";
import { Match } from "effect";
import {
	forgeInfoOptions,
	getReviewQueryOptions,
	getReviewMergeStatusQueryOptions,
	listCIChecksQueryOptions,
	listReviewCommentsQueryOptions,
	listReviewSubmissionsQueryOptions,
} from "#ui/api/queries.ts";
import type {
	ForgeInfo,
	ForgeReview,
	ForgeReviewSubmission,
	ForgeReviewComment,
	ForgeReviewUser,
	ReviewMergeStatus,
	ReviewMergeMethod,
} from "@gitbutler/but-sdk";
import { type QueryClient, queryOptions, useMutation, useQuery } from "@tanstack/react-query";
import * as idb from "idb-keyval";

export const prForgeUrl = (prNo: number, forge: ForgeInfo): string =>
	`${forge.baseUrl}${forge.prUrlPath}${prNo}`;

/**
 * Verify that a persisted review number identifies a merged review: the
 * stored number alone cannot say what became of its review, so the review is
 * fetched and the number is returned until the fetch proves it did NOT merge.
 * Optimistic on purpose — while loading, on a failed fetch, and offline,
 * suppressing the create-PR flow beats flashing it for a landed branch, and
 * it matches the branch row's chip. Null only for a review verified as
 * closed-without-merging, and when no number was given. `enabled` defers the
 * fetch until the consumer actually needs the verdict; the returned value
 * stays live either way, so read it only where review state is shown.
 */
export const useLandedReviewId = (
	projectId: string,
	reviewId: number | null,
	enabled: boolean,
): number | null => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: isMerged } = useQuery({
		...getReviewQueryOptions({ projectId, reviewId: reviewId ?? 0 }),
		enabled: enabled && reviewId !== null && forgeInfo?.capabilities.prService === true,
		select: (review) => review.mergedAt !== null,
	});
	if (reviewId === null) return null;
	return isMerged === false ? null : reviewId;
};

const mergeMethodKey = (projectId: string): string => `pr_merge_method:v1:${projectId}`;

export const mergeMethodQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [projectId, "prMergeMethod"],
		queryFn: async () =>
			(await idb.get<ReviewMergeMethod>(mergeMethodKey(projectId))) ?? ("merge" as const),
	});

export const usePersistMergeMethod = () =>
	useMutation({
		mutationFn: ({ projectId, method }: { projectId: string; method: ReviewMergeMethod }) =>
			idb.set(mergeMethodKey(projectId), method),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(mergeMethodQueryOptions(input.projectId).queryKey, input.method),
	});

/**
 * All fields optional so a record written by an older build still reads.
 *
 * `labels` and `reviewers` are settings for a PR that does not exist yet: the
 * forge takes neither when creating one, so they are held here until there is
 * a review to apply them to.
 */
type DraftPR = {
	title?: string;
	body?: string;
	isDraft?: boolean;
	labels?: Array<string>;
	reviewers?: Array<string>;
};

/** The part of a draft the panel beside the create form owns. */
export type DraftPRExtras = {
	labels: Array<string>;
	reviewers: Array<string>;
};

// Branch name isn't stable identity. Ideally in the future this'd be written to Git metadata.
const draftPRKey = ({ projectId, branchName }: { projectId: string; branchName: string }): string =>
	`pr_draft:v1:${projectId}:${branchName}`;

/** Move a draft PR, if any, from an old branch name to a new one following a rename. */
export const moveDraftPR = async ({
	queryClient,
	projectId,
	oldBranch,
	newBranch,
}: {
	queryClient: QueryClient;
	projectId: string;
	oldBranch: string;
	newBranch: string;
}): Promise<void> => {
	const prevKey = draftPRKey({ projectId, branchName: oldBranch });
	const draft = await idb.get<DraftPR>(prevKey);
	if (!draft) return;

	const newKey = draftPRKey({ projectId, branchName: newBranch });
	await idb.set(newKey, draft);
	queryClient.setQueryData(
		draftPRQueryOptions({ projectId, branchName: newBranch }).queryKey,
		draft,
	);

	await idb.del(prevKey);
	queryClient.removeQueries({
		queryKey: draftPRQueryOptions({ projectId, branchName: oldBranch }).queryKey,
	});
};

export const draftPRQueryOptions = ({
	projectId,
	branchName,
}: {
	projectId: string;
	branchName: string;
}) =>
	queryOptions({
		queryKey: [projectId, "prDraft", branchName],
		queryFn: async () => (await idb.get<DraftPR>(draftPRKey({ projectId, branchName }))) ?? null,
	});

export const usePersistDraftPR = () =>
	useMutation({
		mutationFn: ({
			projectId,
			branchName,
			draft,
		}: {
			projectId: string;
			branchName: string;
			draft: DraftPR;
		}) => idb.set(draftPRKey({ projectId, branchName }), draft),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(
				draftPRQueryOptions({ projectId: input.projectId, branchName: input.branchName }).queryKey,
				input.draft,
			),
	});

export const useDeleteDraftPR = () =>
	useMutation({
		mutationFn: ({ projectId, branchName }: { projectId: string; branchName: string }) =>
			idb.del(draftPRKey({ projectId, branchName })),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(
				draftPRQueryOptions({ projectId: input.projectId, branchName: input.branchName }).queryKey,
				null,
			),
	});

/** Dismissals collapse to "commented", so they never appear as a verdict. */
export type ReviewerVerdict = "approved" | "changesRequested" | "commented" | "awaiting";

export type ReviewerRow = { user: ForgeReviewUser; verdict: ReviewerVerdict };

/**
 * One row per reviewer: everyone still requested (awaiting) plus everyone
 * who submitted a review, carrying their effective verdict. A comment-only
 * submission never overrides an earlier approval or change request, and a
 * dismissal drops the verdict back to commented.
 */
export const reviewerRows = (
	requested: Array<ForgeReviewUser>,
	submissions: Array<ForgeReviewSubmission>,
): Array<ReviewerRow> => {
	const byLogin = new Map<string, ReviewerRow>();
	for (const submission of submissions) {
		if (submission.author === null) continue;
		const existing = byLogin.get(loginKey(submission.author.login));
		const verdict = Match.value(submission.state).pipe(
			Match.withReturnType<ReviewerVerdict>(),
			Match.when("approved", () => "approved"),
			Match.when("changesRequested", () => "changesRequested"),
			Match.when("commented", () => existing?.verdict ?? "commented"),
			Match.when("dismissed", () => "commented"),
			Match.exhaustive,
		);
		byLogin.set(loginKey(submission.author.login), { user: submission.author, verdict });
	}
	for (const user of requested) {
		const key = loginKey(user.login);
		if (!byLogin.has(key)) byLogin.set(key, { user, verdict: "awaiting" });
	}
	return [...byLogin.values()];
};

/** Why the Merge button is disabled, or null when merging is possible. */
const mergeBlockedReason = (
	mergeStatus: Pick<ReviewMergeStatus, "isMergeable" | "mergeableState"> | undefined,
): string | null => {
	if (mergeStatus === undefined) return "Checking mergeability…";
	if (mergeStatus.isMergeable) return null;

	switch (mergeStatus.mergeableState) {
		case "blocked":
			return "Blocked: required approvals or checks are not satisfied";
		case "behind":
			return "Behind the base branch; update the branch first";
		case "dirty":
			return "Merge conflicts with the base branch";
		case "draft":
			return "Draft pull requests cannot be merged";
		case "unknown":
		case "checking":
		case null:
			return "Mergeability not yet determined by the forge";
		default:
			return `Not mergeable (state: ${mergeStatus.mergeableState})`;
	}
};

type ReadinessRow = { label: string; tone: "safe" | "danger" | "warn" | "pending" };
/** @public */
export const mergeReadiness = (
	review: Pick<ForgeReview, "draft" | "mergedAt" | "closedAt"> & {
		reviewers: ReadonlyArray<unknown>;
	},
	mergeStatus: Pick<ReviewMergeStatus, "isMergeable" | "mergeableState"> | undefined,
	checks: AggregateCIStatus | null | undefined,
	verdicts: ReadonlyArray<ReviewerVerdict | "needsCloserLook">,
) => {
	const ready =
		mergeStatus?.isMergeable === true &&
		!review.draft &&
		review.mergedAt === null &&
		review.closedAt === null;
	const changes = verdicts.includes("changesRequested");
	const closerLook = verdicts.includes("needsCloserLook");
	const approved = verdicts.includes("approved");
	const pending = review.reviewers.length;
	const state = mergeStatus?.mergeableState;
	const conflict = state === "dirty";
	const clean = state === "clean" || state === "can_be_merged";
	const checkRow: ReadinessRow =
		checks === "success"
			? { label: "Checks passed", tone: "safe" }
			: checks === "failure" || checks === "cancelled"
				? { label: "Checks failed", tone: "danger" }
				: checks === "action_required"
					? { label: "Checks need attention", tone: "warn" }
					: checks === "in_progress"
						? { label: "Checks running", tone: "warn" }
						: { label: "Checks not reported", tone: "pending" };
	const pendingLabel = `${pending} review${pending === 1 ? "" : "s"} pending`;
	const inputs = [
		changes ? "Changes recommended" : closerLook ? "Needs a closer look" : false,
		pending > 0 && pendingLabel,
		checkRow.tone !== "safe" && checkRow.tone !== "pending" && checkRow.label,
	]
		.filter(Boolean)
		.join(" · ");
	const blocker = ready
		? null
		: review.mergedAt !== null
			? "Already merged"
			: review.closedAt !== null
				? "Pull request is closed"
				: review.draft
					? "Draft pull requests cannot be merged"
					: (state === "blocked" || state === "unstable") && inputs !== ""
						? inputs
						: mergeBlockedReason(mergeStatus);
	const headline = ready
		? "Ready to merge"
		: review.mergedAt !== null
			? "Merged"
			: review.closedAt !== null
				? "Closed"
				: review.draft
					? "Draft"
					: conflict
						? "Blocked on conflicts"
						: state === "blocked" && (changes || closerLook || pending > 0)
							? "Blocked on review"
							: checkRow.tone === "danger" || checkRow.tone === "warn"
								? "Blocked on checks"
								: mergeStatus === undefined
									? "Checking mergeability"
									: "Merge blocked";
	const rows: Array<ReadinessRow> = [
		checkRow,
		{
			label: changes
				? "Changes recommended"
				: closerLook
					? "Needs a closer look"
					: approved
						? "Approved"
						: "No review verdict",
			tone: changes || closerLook ? "warn" : approved ? "safe" : "pending",
		},
		{ label: pendingLabel, tone: "pending" },
		{
			label: conflict ? "Merge conflicts" : clean ? "No conflicts" : "Conflicts not reported",
			tone: conflict ? "danger" : clean ? "safe" : "pending",
		},
	];
	return { ready, blocker, headline, rows };
};

export const useMergeReadiness = (projectId: string, review: ForgeReview) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: mergeStatus, isError } = useQuery({
		...getReviewMergeStatusQueryOptions({ projectId, reviewId: review.number }),
		enabled: !review.draft && review.mergedAt === null && review.closedAt === null,
	});
	const { data: checks } = useQuery({
		...listCIChecksQueryOptions({ projectId, reference: review.sourceBranch, polling: "priority" }),
		enabled: forgeInfo?.capabilities.checks === true,
		select: (data) => data.aggregate?.status ?? null,
	});
	const { data: submissions } = useQuery({
		...listReviewSubmissionsQueryOptions({ projectId, reviewId: review.number }),
		enabled: forgeInfo?.capabilities.reviewComments === true,
	});
	const { data: comments } = useQuery({
		...listReviewCommentsQueryOptions({ projectId, reviewId: review.number }),
		enabled: forgeInfo?.capabilities.reviewComments === true,
	});
	const verdicts = reviewReadinessVerdicts(submissions ?? [], comments ?? []);
	const result = mergeReadiness(review, mergeStatus, checks, verdicts);
	return isError
		? {
				...result,
				ready: false,
				blocker: "Could not check mergeability",
				headline: "Mergeability unavailable",
			}
		: result;
};

export const reviewVerdictBody = (body: string) => {
	const heading =
		/^(?:#{1,6}[ \t]+)?(?:[🟡🔵🟢✅]\uFE0F?[ \t]+)?(?:\*\*)?(Changes recommended|Needs a closer look|Approved)(?:\*\*)?[ \t]*(?:\r?\n|$)/iu.exec(
			body,
		);
	if (!heading) return { body, badge: undefined };
	const key = (heading[1] ?? "").toLowerCase();
	const badge =
		key === "changes recommended"
			? { label: "Changes recommended", variant: "warn" as const }
			: key === "needs a closer look"
				? { label: "Needs a closer look", variant: "pop" as const }
				: { label: "Approved", variant: "safe" as const };
	return { body: body.slice(heading[0].length).replace(/^(?:\r?\n)+/, ""), badge };
};

/** @public */
export const reviewReadinessVerdicts = (
	submissions: ReadonlyArray<
		Pick<ForgeReviewSubmission, "id" | "author" | "body" | "state" | "submittedAt">
	>,
	comments: ReadonlyArray<
		Pick<ForgeReviewComment, "id" | "author" | "body" | "createdAt" | "modifiedAt">
	>,
): Array<ReviewerVerdict | "needsCloserLook"> => {
	type Verdict = ReviewerVerdict | "needsCloserLook";
	type Event = { key: string; at: number; state: Verdict | "dismissed" };
	const bodyVerdict = (body: string): Verdict | undefined => {
		const badge = reviewVerdictBody(body).badge;
		return badge?.variant === "warn"
			? "changesRequested"
			: badge?.variant === "pop"
				? "needsCloserLook"
				: badge?.variant === "safe"
					? "approved"
					: undefined;
	};
	const timestamp = (value: string | null) => {
		const at = Date.parse(value ?? "");
		return Number.isNaN(at) ? 0 : at;
	};
	const events: Array<Event> = [
		...comments.flatMap((comment): Array<Event> => {
			const state = bodyVerdict(comment.body);
			return state === undefined
				? []
				: [
						{
							key: comment.author?.login.toLowerCase() ?? `comment:${String(comment.id)}`,
							at: timestamp(comment.modifiedAt ?? comment.createdAt),
							state,
						},
					];
		}),
		...submissions.map(
			(submission): Event => ({
				key: submission.author?.login.toLowerCase() ?? `submission:${String(submission.id)}`,
				at: timestamp(submission.submittedAt),
				state:
					submission.state === "commented"
						? (bodyVerdict(submission.body ?? "") ?? "commented")
						: submission.state,
			}),
		),
	];
	events.sort((a, b) => a.at - b.at);
	const verdicts = new Map<string, Verdict>();
	for (const event of events) {
		if (event.state === "dismissed") verdicts.set(event.key, "commented");
		else if (event.state !== "commented" || !verdicts.has(event.key))
			verdicts.set(event.key, event.state);
	}
	return Array.from(verdicts.values());
};
