import {
	useAddReviewLabels,
	useRemoveReviewLabel,
	useRequestReview,
	useSetReviewDraftiness,
	useWithdrawReviewRequest,
} from "#ui/api/mutations.ts";
import {
	currentForgeLoginQueryOptions,
	forgeInfoOptions,
	listCIChecksQueryOptions,
	listReviewSubmissionsQueryOptions,
	repoLabelsQueryOptions,
	reviewerCandidatesQueryOptions,
} from "#ui/api/queries.ts";
import { Badge, type BadgeVariant } from "#ui/components/Badge.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { type NativeMenuItem, nativeMenuItem, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { DraftPRExtras } from "#ui/pr.ts";
import { formatAbsoluteTime, formatCompactDuration, formatRelativeTime } from "#ui/time.ts";
import type {
	CiCheck,
	ForgeReview,
	ForgeReviewLabel,
	ForgeReviewSubmission,
	ForgeReviewUser,
} from "@gitbutler/but-sdk";
import { Tooltip } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { type FC, type MouseEvent, type ReactNode, useLayoutEffect, useRef, useState } from "react";
import styles from "./PullRequestPanel.module.css";

type ReviewStatus = "open" | "draft" | "merged" | "closed";

const reviewStatus = (review: ForgeReview): ReviewStatus =>
	review.mergedAt !== null
		? "merged"
		: review.closedAt !== null
			? "closed"
			: review.draft
				? "draft"
				: "open";

const Section: FC<{
	heading: string;
	action?: ReactNode;
	/** Reads as one block with the section above it: no divider, no gap. */
	joined?: boolean;
	children: ReactNode;
}> = (p) => (
	<div className={classes(styles.section, p.joined === true && styles.sectionJoined)}>
		<div className={styles.sectionHeader}>
			<h4 className={classes("text-12", styles.heading)}>{p.heading}</h4>
			{p.action}
		</div>
		{p.children}
	</div>
);

/**
 * A native menu with no items opens as an empty rectangle, which reads as a
 * broken button rather than an empty list — say why instead.
 */
const orEmptyNotice = (items: Array<NativeMenuItem>, notice: string): Array<NativeMenuItem> =>
	items.length > 0 ? items : [nativeMenuItem({ label: notice, enabled: false })];

const pickerButton = (label: string, onClick: (evt: MouseEvent<HTMLButtonElement>) => void) => (
	<button
		aria-label={label}
		className={getButtonClassName({ variant: "ghost", size: "small", iconOnly: true })}
		onClick={onClick}
		type="button"
	>
		<Icon name="plus" />
	</button>
);

/** Muted stand-ins shown while a section has nothing in it yet. */
const PeoplePlaceholder: FC = () => (
	<div className={styles.placeholderPeople}>
		{[100, 160].map((width) => (
			<div key={width} className={styles.placeholderRow}>
				<span className={classes(styles.placeholderShape, styles.placeholderAvatar)} />
				<span
					className={classes(styles.placeholderShape, styles.placeholderBar)}
					style={{ width }}
				/>
			</div>
		))}
	</div>
);

const LabelsPlaceholder: FC = () => (
	<div className={styles.placeholderLabels}>
		{[70, 50, 90].map((width) => (
			<span
				key={width}
				className={classes(styles.placeholderShape, styles.placeholderLabel)}
				style={{ width }}
			/>
		))}
	</div>
);

const ReviewUser: FC<{ user: ForgeReviewUser }> = ({ user }) => (
	<div className={classes("text-13", styles.user)} title={user.name ?? user.login}>
		{user.avatarUrl !== null ? (
			<img src={user.avatarUrl} className={styles.avatar} alt="" />
		) : (
			<span className={styles.avatar} />
		)}
		<span className={styles.userLogin}>{user.login}</span>
	</div>
);

const Label: FC<{ label: ForgeReviewLabel }> = ({ label }) => {
	// GitHub sends bare hex color codes, GitLab prefixes them with `#`.
	const color =
		label.color === null ? null : label.color.startsWith("#") ? label.color : `#${label.color}`;

	return (
		<Badge
			variant="lightGray"
			className={color === null ? undefined : styles.label}
			style={color === null ? undefined : { "--label-color": color }}
			title={label.description ?? undefined}
		>
			{label.name}
		</Badge>
	);
};

const CopyableBranch: FC<{ name: string }> = ({ name }) => {
	const [copied, setCopied] = useState(false);
	const resetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

	const handleCopy = () => {
		void window.lite.clipboardWriteText(name);
		setCopied(true);

		if (resetTimeoutRef.current !== null) clearTimeout(resetTimeoutRef.current);
		resetTimeoutRef.current = setTimeout(() => setCopied(false), 1500);
	};

	useLayoutEffect(
		() => () => {
			if (resetTimeoutRef.current !== null) clearTimeout(resetTimeoutRef.current);
		},
		[],
	);

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				className={styles.sourceBranch}
				onClick={handleCopy}
				render={<button type="button" aria-label="Copy branch name" />}
			>
				{copied ? "Copied!" : name}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>Copy branch name</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

/**
 * The side panel while a PR is still being drafted: what the PR would be made
 * of, so the summary the form asks for can be written against something.
 *
 * Reviewers and labels are absent rather than shown empty — `publish_review`
 * takes neither, so there would be nothing to set until the PR exists.
 */
export const NewPullRequestPanel: FC<{
	projectId: string;
	sourceBranch: string;
	/** Unknown while the branch is completely unpushed, which also blocks submit. */
	targetBranch: string | undefined;
	/** Held here until the PR exists; see {@link DraftPRExtras}. */
	extras: DraftPRExtras;
	onExtrasChange: (extras: DraftPRExtras) => void;
}> = ({ projectId, sourceBranch, targetBranch, extras, onExtrasChange }) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const canManage = forgeInfo?.capabilities.reviewManagement === true;
	const { data: repoLabels } = useQuery({
		...repoLabelsQueryOptions(projectId),
		enabled: canManage,
	});
	const { data: reviewerCandidates } = useQuery({
		...reviewerCandidatesQueryOptions(projectId),
		enabled: canManage,
	});
	const { data: currentLogin } = useQuery(currentForgeLoginQueryOptions(projectId));

	// Cached data still reads after the capability flips off, so gate on the
	// capability rather than on the cache being populated.
	const canPickLabels = canManage && repoLabels !== undefined;
	const canPickReviewers = canManage && reviewerCandidates !== undefined;

	const toggle = (list: Array<string>, value: string): Array<string> =>
		list.includes(value) ? list.filter((entry) => entry !== value) : [...list, value];

	const openLabelMenu = (evt: MouseEvent<HTMLButtonElement>) => {
		if (!canPickLabels) return;
		void showNativeMenuFromTrigger(
			evt.currentTarget,
			orEmptyNotice(
				repoLabels.map((label) =>
					nativeMenuItem({
						label: label.name,
						checked: extras.labels.includes(label.name),
						onSelect: () =>
							onExtrasChange({ ...extras, labels: toggle(extras.labels, label.name) }),
					}),
				),
				"This repository has no labels",
			),
		);
	};

	const openReviewerMenu = (evt: MouseEvent<HTMLButtonElement>) => {
		if (!canPickReviewers) return;
		void showNativeMenuFromTrigger(
			evt.currentTarget,
			orEmptyNotice(
				reviewerCandidates
					// The author can't review their own PR, so a solo repository
					// leaves nothing to pick.
					.filter((candidate) => candidate.login !== currentLogin)
					.map((candidate) =>
						nativeMenuItem({
							label: candidate.login,
							checked: extras.reviewers.includes(candidate.login),
							onSelect: () =>
								onExtrasChange({ ...extras, reviewers: toggle(extras.reviewers, candidate.login) }),
						}),
					),
				"No one else can be asked to review",
			),
		);
	};

	const pickedReviewers = extras.reviewers.map((login) => ({
		login,
		user: reviewerCandidates?.find((candidate) => candidate.login === login),
	}));
	const pickedLabels = extras.labels.map(
		(name) =>
			repoLabels?.find((label) => label.name === name) ?? { name, color: null, description: null },
	);

	return (
		<aside className={styles.panel}>
			<Section
				heading="Reviewers"
				action={canPickReviewers && pickerButton("Request a review", openReviewerMenu)}
			>
				{pickedReviewers.length === 0 ? (
					<PeoplePlaceholder />
				) : (
					pickedReviewers.map(({ login, user }) =>
						user === undefined ? (
							<span key={login} className="text-13">
								{login}
							</span>
						) : (
							<ReviewUser key={login} user={user} />
						),
					)
				)}
			</Section>

			<Section
				heading="Labels"
				action={canPickLabels && pickerButton("Edit labels", openLabelMenu)}
			>
				{pickedLabels.length === 0 ? (
					<LabelsPlaceholder />
				) : (
					<div className={styles.labels}>
						{pickedLabels.map((label) => (
							<Label key={label.name} label={label} />
						))}
					</div>
				)}
			</Section>

			<Section heading="Branches">
				<div className={classes("text-13", styles.branches)}>
					<CopyableBranch name={sourceBranch} />
					{targetBranch !== undefined && (
						<>
							<span className={styles.branchArrow}>→</span>
							<span className={styles.targetBranch}>{targetBranch}</span>
						</>
					)}
				</div>
			</Section>
		</aside>
	);
};

const reportOpenFailure = (error: unknown) => {
	// oxlint-disable-next-line no-console
	console.error(error);
};

const openLinkExternally = (evt: MouseEvent<HTMLAnchorElement>): void => {
	evt.preventDefault();
	window.lite.openInWebBrowser(evt.currentTarget.href).catch(reportOpenFailure);
};

/** A failing check, carrying the dot colour its conclusion earns. */
type ProblemCheck = { check: CiCheck; tone: "danger" | "warn" | "muted" };

/** Wall time from a check's start to its completion, once both are known. */
const checkDuration = (check: CiCheck): string | null => {
	const completedAt = typeof check.status === "string" ? null : check.status.complete.completed_at;
	if (check.startedAt === null || completedAt === null) return null;
	const ms = Date.parse(completedAt) - Date.parse(check.startedAt);
	return Number.isNaN(ms) || ms < 0 ? null : formatCompactDuration(ms);
};

const ProblemCheckRow: FC<{ problem: ProblemCheck }> = ({ problem: { check, tone } }) => {
	const duration = checkDuration(check);

	return (
		<a
			href={check.htmlUrl}
			onClick={openLinkExternally}
			className={classes("text-12", styles.checkRow)}
		>
			<span
				className={classes(
					styles.checkDot,
					Match.value(tone).pipe(
						Match.when("danger", () => styles.checkDotDanger),
						Match.when("warn", () => styles.checkDotWarn),
						Match.when("muted", () => styles.checkDotMuted),
						Match.exhaustive,
					),
				)}
			/>
			<span className={styles.checkName}>{check.name}</span>
			<span className={styles.checkMeta}>
				{duration !== null && (
					<>
						{duration}
						<span>•</span>
					</>
				)}
				<Icon name="arrow-up-right" size={14} />
			</span>
		</a>
	);
};

/**
 * CI at a glance: a bar apportioned between the checks that passed, are still
 * running and failed, the same three as counts, and a row per failing check.
 */
const ChecksSection: FC<{ projectId: string; reference: string }> = ({ projectId, reference }) => {
	const { data } = useQuery(
		listCIChecksQueryOptions({ projectId, reference, polling: "priority" }),
	);
	const aggregate = data?.aggregate ?? null;
	if (aggregate === null) return null;

	// Cancelled checks didn't pass either, so they join the problem list — with
	// a muted dot, since nothing went wrong so much as stopped.
	const problems: Array<ProblemCheck> = [
		...aggregate.failure.map((check): ProblemCheck => ({ check, tone: "danger" })),
		...aggregate.timedOut.map((check): ProblemCheck => ({ check, tone: "danger" })),
		...aggregate.actionRequired.map((check): ProblemCheck => ({ check, tone: "warn" })),
		...aggregate.cancelled.map((check): ProblemCheck => ({ check, tone: "muted" })),
	];
	// A check of unknown state hasn't resolved, so it waits with the pending.
	const pending = aggregate.inProgress.length + aggregate.queued.length + aggregate.unknown.length;
	const passed = aggregate.success.length + aggregate.neutral.length;
	const skipped = aggregate.skipped.length;
	// Skipped checks ran nothing, so they're counted but not apportioned.
	const segments = [
		{ key: "passed", count: passed, className: styles.barPassed },
		{ key: "pending", count: pending, className: styles.barPending },
		{ key: "failed", count: problems.length, className: styles.barFailed },
	].filter((segment) => segment.count > 0);

	return (
		<Section heading="Checks">
			<div className={styles.checks}>
				<div className={styles.checksSummary}>
					<div className={styles.checksBar}>
						{segments.map((segment) => (
							<span
								key={segment.key}
								className={classes(styles.barSegment, segment.className)}
								style={{ flexGrow: segment.count }}
							/>
						))}
					</div>

					<div className={classes("text-12", styles.checksCounts)}>
						{problems.length > 0 && (
							<span className={styles.countFailed}>{problems.length} failed</span>
						)}
						{passed > 0 && (
							<span className={styles.countPassed}>
								{problems.length === 0 && pending === 0
									? `All ${passed} passed`
									: `${passed} passed`}
							</span>
						)}
						{pending > 0 && <span className={styles.countPending}>{pending} pending</span>}
						{skipped > 0 && <span className={styles.countSkipped}>{skipped} skipped</span>}
					</div>
				</div>

				{problems.length > 0 && (
					<>
						<div className={styles.checksDivider} />
						<div className={styles.checksList}>
							{problems.map((problem) => (
								<ProblemCheckRow key={problem.check.id} problem={problem} />
							))}
						</div>
					</>
				)}
			</div>
		</Section>
	);
};

/** Dismissals collapse to "commented", so they never appear as a verdict. */
type ReviewerVerdict = "approved" | "changesRequested" | "commented" | "awaiting";

type ReviewerRow = { user: ForgeReviewUser; verdict: ReviewerVerdict };

/**
 * One row per reviewer: everyone still requested (awaiting) plus everyone
 * who submitted a review, carrying their effective verdict. A comment-only
 * submission never overrides an earlier approval or change request, and a
 * dismissal drops the verdict back to commented.
 */
const reviewerRows = (
	requested: Array<ForgeReviewUser>,
	submissions: Array<ForgeReviewSubmission>,
): Array<ReviewerRow> => {
	const byLogin = new Map<string, ReviewerRow>();
	for (const submission of submissions) {
		if (submission.author === null) continue;
		const existing = byLogin.get(submission.author.login);
		const verdict = Match.value(submission.state).pipe(
			Match.withReturnType<ReviewerVerdict>(),
			Match.when("approved", () => "approved"),
			Match.when("changesRequested", () => "changesRequested"),
			Match.when("commented", () => existing?.verdict ?? "commented"),
			Match.when("dismissed", () => "commented"),
			Match.exhaustive,
		);
		byLogin.set(submission.author.login, { user: submission.author, verdict });
	}
	for (const user of requested)
		if (!byLogin.has(user.login)) byLogin.set(user.login, { user, verdict: "awaiting" });
	return [...byLogin.values()];
};

const verdictBits = (verdict: ReviewerVerdict): [IconName, string, string] =>
	Match.value(verdict).pipe(
		Match.withReturnType<[IconName, string, string]>(),
		Match.when("approved", () => ["tick-circle", "var(--fill-safe-bg)", "Approved"]),
		Match.when("changesRequested", () => [
			"cross-circle",
			"var(--fill-danger-bg)",
			"Requested changes",
		]),
		Match.when("commented", () => ["eye", "var(--text-3)", "Commented"]),
		Match.when("awaiting", () => ["clock", "var(--text-3)", "Awaiting review"]),
		Match.exhaustive,
	);

export const PullRequestPanel: FC<{ projectId: string; review: ForgeReview }> = ({
	projectId,
	review,
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: reviewers } = useQuery({
		...listReviewSubmissionsQueryOptions({ projectId, reviewId: review.number }),
		// Fail open: an older backend without the field can still read comments.
		enabled: forgeInfo?.capabilities.reviewComments !== false,
		select: (submissions) => reviewerRows(review.reviewers, submissions),
	});
	// Until submissions load, show the requested reviewers without verdicts.
	const reviewerList =
		reviewers ?? review.reviewers.map((user): ReviewerRow => ({ user, verdict: "awaiting" }));

	const canManage = forgeInfo?.capabilities.reviewManagement === true && review.mergedAt === null;
	const { data: repoLabels } = useQuery({
		...repoLabelsQueryOptions(projectId),
		enabled: canManage,
	});
	const { data: reviewerCandidates } = useQuery({
		...reviewerCandidatesQueryOptions(projectId),
		enabled: canManage,
	});
	const { mutate: addReviewLabels } = useAddReviewLabels();
	const { mutate: removeReviewLabel } = useRemoveReviewLabel();
	const { mutate: requestReview } = useRequestReview();
	const { mutate: withdrawReviewRequest } = useWithdrawReviewRequest();
	const { isPending: isDraftinessPending, mutate: setReviewDraftiness } = useSetReviewDraftiness();

	// Neither a merged nor a closed review can change draftiness.
	const canToggleDraft = review.mergedAt === null && review.closedAt === null;

	// The queries stop fetching when canManage flips off, but cached data
	// still reads — gate the pickers on manageability, not cache presence.
	const canPickLabels = canManage && repoLabels !== undefined;
	const canPickReviewers = canManage && reviewerCandidates !== undefined;

	const openLabelMenu = (evt: MouseEvent<HTMLButtonElement>) => {
		if (!canPickLabels) return;
		void showNativeMenuFromTrigger(
			evt.currentTarget,
			repoLabels.map((label) => {
				const applied = review.labels.some((existing) => existing.name === label.name);
				return nativeMenuItem({
					label: label.name,
					checked: applied,
					onSelect: () =>
						applied
							? removeReviewLabel({ projectId, reviewId: review.number, label: label.name })
							: addReviewLabels({ projectId, reviewId: review.number, labels: [label.name] }),
				});
			}),
		);
	};

	const openReviewerMenu = (evt: MouseEvent<HTMLButtonElement>) => {
		if (!canPickReviewers) return;
		void showNativeMenuFromTrigger(
			evt.currentTarget,
			reviewerCandidates
				// The author can't review their own PR.
				.filter((candidate) => candidate.login !== review.author?.login)
				.map((candidate) => {
					const requested = review.reviewers.some((reviewer) => reviewer.login === candidate.login);
					return nativeMenuItem({
						label: candidate.login,
						checked: requested,
						onSelect: () =>
							requested
								? withdrawReviewRequest({
										projectId,
										reviewId: review.number,
										logins: [candidate.login],
									})
								: requestReview({
										projectId,
										reviewId: review.number,
										logins: [candidate.login],
									}),
					});
				}),
		);
	};

	const [statusLabel, statusVariant, statusIcon] = Match.value(reviewStatus(review)).pipe(
		Match.withReturnType<[string, BadgeVariant, IconName]>(),
		Match.when("open", () => ["Open", "safe", "pr"]),
		Match.when("draft", () => ["Draft", "lightGray", "pr-draft"]),
		Match.when("merged", () => ["Merged", "purple", "branch-merge"]),
		Match.when("closed", () => ["Closed", "danger", "pr-close"]),
		Match.exhaustive,
	);

	const createdAtMs = review.createdAt === null ? null : Date.parse(review.createdAt);
	const modifiedAtMs = review.modifiedAt === null ? null : Date.parse(review.modifiedAt);
	// The forge stamps modified_at on any activity, so creation itself can
	// leave the two a moment apart; only a real gap is worth a row.
	const showUpdated =
		modifiedAtMs !== null && (createdAtMs === null || modifiedAtMs - createdAtMs > 60_000);

	const handleOpen = (evt: MouseEvent<HTMLAnchorElement>): void => {
		evt.preventDefault();
		window.lite.openInWebBrowser(review.htmlUrl).catch(reportOpenFailure);
	};

	return (
		<aside className={styles.panel}>
			<Section
				heading="Status"
				action={
					<a
						href={review.htmlUrl}
						onClick={handleOpen}
						className={classes("text-12", styles.link, styles.prLink)}
					>
						{review.unitSymbol}
						{review.number}
						<Icon name="arrow-up-right" size={12} />
					</a>
				}
			>
				<div className={styles.statusRow}>
					<Badge variant={statusVariant} size="large">
						<Icon name={statusIcon} size={12} />
						{statusLabel}
					</Badge>
					{canToggleDraft && (
						<button
							className={getButtonClassName({ variant: "outline", size: "small" })}
							disabled={isDraftinessPending}
							onClick={() =>
								setReviewDraftiness({
									projectId,
									reviewId: review.number,
									draft: !review.draft,
								})
							}
							type="button"
						>
							{review.draft ? "Mark as ready" : "Convert to draft"}
						</button>
					)}
				</div>
			</Section>

			<Section
				heading="Reviewers"
				action={canPickReviewers && pickerButton("Request a review", openReviewerMenu)}
			>
				{reviewerList.length === 0 ? (
					<PeoplePlaceholder />
				) : (
					reviewerList.map(({ user, verdict }) => {
						const [icon, color, label] = verdictBits(verdict);
						return (
							<div key={user.id} className={styles.reviewerRow} title={label}>
								<ReviewUser user={user} />
								<Icon name={icon} style={{ color }} size={15} />
							</div>
						);
					})
				)}
			</Section>

			<Section
				heading="Labels"
				action={canPickLabels && pickerButton("Edit labels", openLabelMenu)}
			>
				{review.labels.length === 0 ? (
					<LabelsPlaceholder />
				) : (
					<div className={styles.labels}>
						{review.labels.map((label) => (
							<Label key={label.name} label={label} />
						))}
					</div>
				)}
			</Section>

			{forgeInfo?.capabilities.checks === true && (
				<ChecksSection projectId={projectId} reference={review.sourceBranch} />
			)}

			<Section heading="Branches">
				<div className={classes("text-13", styles.branches)}>
					<CopyableBranch name={review.sourceBranch} />
					<span className={styles.branchArrow}>→</span>
					<span className={styles.targetBranch}>{review.targetBranch}</span>
				</div>
			</Section>

			{review.author !== null && (
				<Section heading="Author">
					<ReviewUser user={review.author} />
				</Section>
			)}

			{createdAtMs !== null && (
				<Section heading="Created">
					<span className={classes("text-13", styles.created)}>
						{formatRelativeTime(createdAtMs)}, {formatAbsoluteTime(createdAtMs)}
					</span>
				</Section>
			)}

			{showUpdated && (
				<Section heading="Updated" joined>
					<span className={classes("text-13", styles.created)}>
						{formatRelativeTime(modifiedAtMs)}, {formatAbsoluteTime(modifiedAtMs)}
					</span>
				</Section>
			)}
		</aside>
	);
};
