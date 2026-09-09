import {
	type PushBeforePublish,
	useAddReviewReaction,
	useGeneratePrDescription,
	useMergeReview,
	usePublishReview,
	useRemoveReviewReaction,
	useSetReviewAutoMerge,
	useSetReviewDraftiness,
	useUpdateReview,
	useWorkspaceBranchAndAncestorsPush,
} from "#ui/api/mutations.ts";
import {
	aiConfigurationQueryOptions,
	branchDetailsQueryOptions,
	currentForgeLoginQueryOptions,
	forgeInfoOptions,
	getReviewMergeStatusQueryOptions,
	listReviewReactionsQueryOptions,
	listReviewTimelineEventsQueryOptions,
} from "#ui/api/queries.ts";
import {
	Reactions,
	tallyReactions,
} from "#ui/routes/project/$id/workspace/PullRequestReactions.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Clamped } from "#ui/components/Clamped.tsx";
import { classes } from "#ui/components/classes.ts";
import { DropdownButton } from "#ui/components/DropdownButton.tsx";
import { FieldControlStyles, FieldRootStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Markdown } from "#ui/components/Markdown.tsx";
import { ReviewUser } from "#ui/routes/project/$id/workspace/PullRequestPanel.tsx";
import { formatAbsoluteTime, formatRelativeTime } from "#ui/time.ts";
import { branchDetailsParams } from "#ui/branch.ts";
import { MarkdownAttachments } from "#ui/components/MarkdownAttachments.tsx";
import { MarkdownToolbar } from "#ui/components/MarkdownToolbar.tsx";
import { SwitchButton } from "#ui/components/SwitchButton.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { pullRequestHotkeys } from "#ui/hotkeys.ts";
import { prDescriptionGenerationButtonState } from "#ui/pr-description-generation.ts";
import { projectAiSettingsQueryOptions } from "#ui/project-ai-settings.ts";
import {
	nativeMenuItem,
	nativeMenuItemsFromGroups,
	showNativeMenuFromTrigger,
} from "#ui/native-menu.ts";
import {
	draftPRQueryOptions,
	mergeMethodQueryOptions,
	useDeleteDraftPR,
	usePersistDraftPR,
	usePersistMergeMethod,
} from "#ui/pr.ts";
import { type FocusScope, useAutofocusScope } from "#ui/focus-scopes.ts";
import { Field, Tooltip } from "@base-ui/react";
import type { ForgeReview, ReviewMergeMethod, ReviewMergeStatus } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useHotkey } from "@tanstack/react-hotkeys";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import {
	type FC,
	type ReactNode,
	type SubmitEvent,
	Suspense,
	useEffect,
	useRef,
	useState,
} from "react";
import styles from "./PullRequestForm.module.css";

/**
 * The building blocks of the Pull Request tab, defined bottom-up:
 *
 * - {@link PullRequestForm} — the create/edit form (also standalone for
 *   branches without a PR yet)
 * - {@link PullRequestDescription} — the rendered title/body view that
 *   flips into the form when edit mode is on
 * - {@link PullRequestPrimaryAction} — the Edit/auto-merge/Merge button
 *   row; note it mounts in the details *header*, not the tab body
 *
 * `BranchDetails` (Details.tsx) owns the tab switching and edit-mode state
 * and composes these with `PullRequestPanel` and `PullRequestComments`.
 */

/**
 * Title/description form for creating a PR or editing an existing one.
 *
 * Unsubmitted input persists to idb per project+branch (survives restarts,
 * follows renames), and is dropped again once the fields match the remote —
 * clearing them by hand is what discards a draft. When the remote PR changes
 * underneath an *untouched* form, the fields follow the remote; once locally
 * edited, local wins. With `onCancel` set, the footer gains a Cancel button
 * that discards edits and leaves edit mode.
 */
export const PullRequestForm: FC<{
	projectId: string;
	sourceBranch: string;
	reviewId: number | null;
	title: string | null;
	body: string | null;
	canSubmit: boolean;
	/**
	 * For a new PR, the push it has to wait for, or null when the branch is
	 * already on the remote. Never read when editing an existing PR.
	 */
	pushFirst?: PushBeforePublish | null;
	onAfterSubmit?: () => void;
	/** Adds a Cancel button that discards edits and calls this. */
	onCancel?: () => void;
	/**
	 * Called with the new PR's number right after it is created, for the
	 * settings the forge cannot take at creation time (labels, reviewers,
	 * auto-merge). Never called when editing an existing PR.
	 */
	afterPublish?: (reviewId: number) => void;
	/**
	 * Which field takes focus when the form mounts: the title by default, the
	 * description when the user came here to add one.
	 */
	autofocus?: "title" | "body";
}> = ({
	projectId,
	sourceBranch,
	reviewId,
	title,
	body,
	canSubmit,
	pushFirst = null,
	onAfterSubmit,
	onCancel,
	afterPublish,
	autofocus = "title",
}) => {
	const { isPending: isPushPending, mutateAsync: pushBranchAndAncestors } =
		useWorkspaceBranchAndAncestorsPush(projectId);
	const { isPending: isPublishReviewPending, mutate: publishReview } = usePublishReview(projectId);
	const { isPending: isUpdateReviewPending, mutate: updateReview } = useUpdateReview(projectId);
	const formRef = useRef<HTMLFormElement | null>(null);
	const bodyRef = useRef<HTMLTextAreaElement | null>(null);
	/** Drives the rule under the toolbar, so text never slides under it bare. */
	const [bodyScrolled, setBodyScrolled] = useState(false);

	const remoteOrEmptyDocument = {
		title: title ?? "",
		body: body ?? "",
	};
	const { data: persistedDocument } = useSuspenseQuery(
		draftPRQueryOptions({ projectId, branchName: sourceBranch }),
	);
	const [localDocument, setLocalDocument] = useState({
		title: persistedDocument?.title ?? title ?? "",
		body: persistedDocument?.body ?? body ?? "",
		isDraft: persistedDocument?.isDraft ?? false,
	});
	const { data: isAiConfigured = false } = useQuery({
		...aiConfigurationQueryOptions,
		select: (configuration) => configuration.isConfigured,
	});
	const { data: isProjectAiEnabled = false } = useQuery({
		...projectAiSettingsQueryOptions(projectId),
		select: (settings) => settings.enabled,
	});
	const { data: branchDetails } = useQuery(
		branchDetailsQueryOptions({ projectId, ...branchDetailsParams(sourceBranch) }),
	);
	const { isPending: isGenerating, mutate: generateDescriptionMutation } =
		useGeneratePrDescription();
	/**
	 * The document as of the latest render, for callbacks that fire long after
	 * the render that created them — an upload or a generated answer landing.
	 */
	const latestDocument = useRef(localDocument);
	useEffect(() => {
		latestDocument.current = localDocument;
	});
	const { mutate: persistDraftPR } = usePersistDraftPR();
	const { mutate: deleteDraftPR } = useDeleteDraftPR();

	const isNew = reviewId === null;
	const isAnyPending = isPushPending || isPublishReviewPending || isUpdateReviewPending;
	// A forge refuses a PR whose head adds nothing to its base, so an empty
	// branch cannot open one — and pushing it first would only leave an empty
	// branch on the remote. The form stays live for drafting ahead of the
	// commits; only the button waits. Unknown while the details load: not
	// loaded is not the same as empty.
	const noCommits = isNew && branchDetails !== undefined && branchDetails.commits.length === 0;
	const hasChanges =
		localDocument.title !== remoteOrEmptyDocument.title ||
		localDocument.body !== remoteOrEmptyDocument.body ||
		(isNew && localDocument.isDraft);

	// Reset to latest remote data if we haven't locally diverged yet.
	const [prevRemote, setPrevRemote] = useState(remoteOrEmptyDocument);
	const remoteHasUpdated =
		prevRemote.title !== remoteOrEmptyDocument.title ||
		prevRemote.body !== remoteOrEmptyDocument.body;
	if (remoteHasUpdated) {
		setPrevRemote(remoteOrEmptyDocument);

		const localHasDiverged =
			localDocument.title !== prevRemote.title || localDocument.body !== prevRemote.body;
		if (!localHasDiverged) {
			setLocalDocument((prev) => ({
				...prev,
				...remoteOrEmptyDocument,
			}));
		}
	}

	const handleBlur = () => {
		if (hasChanges) {
			persistDraftPR({
				projectId,
				branchName: sourceBranch,
				// Merged, not replaced: the panel beside this form owns the
				// record's other fields and would otherwise be wiped.
				draft: { ...persistedDocument, ...localDocument },
			});
		} else if (persistedDocument) {
			deleteDraftPR({ projectId, branchName: sourceBranch });
		}
	};

	const handleReset = () => {
		const resetDocument = { ...remoteOrEmptyDocument, isDraft: false };
		setLocalDocument(resetDocument);
		deleteDraftPR({ projectId, branchName: sourceBranch });
	};

	const generationButton = prDescriptionGenerationButtonState({
		enabled: isProjectAiEnabled,
		configured: isAiConfigured,
		busy: isGenerating || isAnyPending,
		commitCount: branchDetails?.commits.length,
	});

	const generateDescription = () => {
		if (isGenerating) return;

		generateDescriptionMutation(
			{
				projectId,
				sourceBranch,
				previousTitle: localDocument.title,
				previousBody: localDocument.body,
				// Streamed straight into the DOM: routing every token through state
				// would re-render the form, and that render would overwrite the
				// textarea from the state the DOM is deliberately running ahead of.
				// The title is held back until the end for the same reason — it is
				// one line, so it costs nothing to wait.
				onBody: (body) => {
					if (bodyRef.current !== null) bodyRef.current.value = body;
				},
			},
			{
				onSuccess: ({ title, body }) => {
					if (bodyRef.current !== null) bodyRef.current.value = body;
					// Built from the document as it stands now, not as it was at click
					// time: the Draft switch and the title stay live while the answer
					// streams, and this both overwrites state and persists it.
					const current = latestDocument.current;
					const generated = {
						...current,
						// An answer with no title at all leaves the typed one alone.
						title: title === "" ? current.title : title,
						body,
					};
					setLocalDocument(generated);
					// Persisted here rather than left to the form's blur: generating
					// takes no focus that could later leave the form — the button
					// disables itself while it runs, which drops focus outright — so
					// switching tabs would unmount the form having saved nothing.
					persistDraftPR({
						projectId,
						branchName: sourceBranch,
						draft: { ...persistedDocument, ...generated },
					});
				},
				// A failed stream restores the previous body in the DOM only;
				// state never moved, so there is nothing to put back here.
			},
		);
	};

	const handleSubmit = async (evt: SubmitEvent<HTMLFormElement>): Promise<void> => {
		evt.preventDefault();
		if (!canSubmit || noCommits || isAnyPending || localDocument.title.trim() === "") return;

		if (reviewId === null) {
			// A forge only opens a review on a branch it has, so the branch and
			// its ancestors go up first when any still has something to push.
			// The PR's source is then the name the branch landed under on the
			// remote, which differs from the local one when the branch tracks
			// another remote.
			let remoteSourceBranch = sourceBranch;
			if (pushFirst !== null) {
				// The push hook already toasts its own failure.
				const pushed = await pushBranchAndAncestors({
					projectId,
					branch: pushFirst.branch,
					withForce: pushFirst.withForce,
					skipForcePushProtection: false,
					runHooks: true,
					pushOpts: [],
				}).catch(() => null);
				if (pushed === null) return;
				remoteSourceBranch =
					pushed.branchToRemote.find(([branch]) => branch === sourceBranch)?.[2] ?? sourceBranch;
			}
			publishReview(
				{
					projectId,
					params: {
						title: localDocument.title,
						body: localDocument.body,
						draft: localDocument.isDraft,
						localBranch: sourceBranch,
						sourceBranch: remoteSourceBranch,
					},
				},
				{
					onSuccess: (outcome) => {
						deleteDraftPR({ projectId, branchName: sourceBranch });
						afterPublish?.(outcome.review.number);
					},
				},
			);
		} else {
			updateReview(
				{
					projectId,
					reviewId,
					title: localDocument.title,
					body: localDocument.body,
					state: null,
					targetBase: null,
				},
				{
					onSuccess: () => {
						deleteDraftPR({ projectId, branchName: sourceBranch });
						onAfterSubmit?.();
					},
				},
			);
		}
	};

	useHotkey(pullRequestHotkeys.update.hotkey, () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		enabled: !isAnyPending && hasChanges,
		target: formRef,
	});

	return (
		// oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- Used for persistence, not UI per se.
		<form
			ref={formRef}
			className={styles.prForm}
			onBlur={handleBlur}
			onSubmit={(evt) => void handleSubmit(evt)}
		>
			{/* Both fields name themselves in the placeholder, as designed, so
			    they carry an aria-label instead of a visible one. */}
			<Field.Root render={<FieldRootStyles />}>
				<Field.Control
					render={<FieldControlStyles />}
					aria-label="Pull request title"
					data-focus-scope={"pr" satisfies FocusScope}
					ref={useAutofocusScope(autofocus === "title")}
					name="title"
					onChange={(evt) => setLocalDocument({ ...localDocument, title: evt.currentTarget.value })}
					placeholder="PR title"
					required
					value={localDocument.title}
				/>
			</Field.Root>

			<div className={styles.descriptionEditor} data-body-scrolled={bodyScrolled || undefined}>
				<MarkdownToolbar
					className={styles.descriptionToolbar}
					disabled={isAnyPending}
					onInput={(nextBody) => setLocalDocument({ ...localDocument, body: nextBody })}
					targetRef={bodyRef}
				/>

				<textarea
					aria-label="Pull request description"
					className={classes("text-13", "text-body", styles.descriptionInput)}
					name="body"
					onChange={(evt) => setLocalDocument({ ...localDocument, body: evt.currentTarget.value })}
					// Only the flip re-renders: React bails out of an unchanged state.
					onScroll={(evt) => setBodyScrolled(evt.currentTarget.scrollTop > 0)}
					placeholder="PR description"
					ref={useMergedRefs(bodyRef, useAutofocusScope(autofocus === "body"))}
					value={localDocument.body}
				/>

				<div className={styles.descriptionFooter}>
					<div className={styles.footerRow}>
						<div className={styles.footerStart}>
							<MarkdownAttachments
								disabled={isAnyPending}
								// An upload can land long after the click, so this updates from
								// the current document, not the one captured at click time.
								onInput={(nextBody) => setLocalDocument((prev) => ({ ...prev, body: nextBody }))}
								targetRef={bodyRef}
							/>
							<div aria-hidden className={styles.footerSeparator} />
							<Tooltip.Root>
								{/* Disabled buttons swallow hover, so the wrapper span carries the tooltip. */}
								<Tooltip.Trigger render={<span className={styles.disabledActionWrap} />}>
									<button
										aria-label="Generate title and description"
										className={getButtonClassName({ variant: "ghost", iconOnly: true })}
										disabled={generationButton.disabled}
										onClick={generateDescription}
										type="button"
									>
										<Icon name={isGenerating ? "spinner" : "ai-text"} />
									</button>
								</Tooltip.Trigger>
								<Tooltip.Portal>
									<Tooltip.Positioner sideOffset={4}>
										<Tooltip.Popup render={<TooltipPopup />}>
											{generationButton.hint ?? "Generate title and description"}
										</Tooltip.Popup>
									</Tooltip.Positioner>
								</Tooltip.Portal>
							</Tooltip.Root>
						</div>

						<div className={styles.footerEnd}>
							{isNew && (
								<>
									<SwitchButton
										label="Draft"
										checked={localDocument.isDraft}
										disabled={isAnyPending}
										name="isDraft"
										onCheckedChange={(isDraft) => setLocalDocument({ ...localDocument, isDraft })}
									/>
									<div aria-hidden className={styles.footerSeparator} />
								</>
							)}

							<div className={styles.footerButtons}>
								{/* Only edit mode offers a way out: clearing the fields by hand
								    already drops the persisted draft on blur, so a Reset button
								    would just be a destructive shortcut for that. */}
								{onCancel !== undefined && (
									<button
										className={getButtonClassName({})}
										disabled={isAnyPending}
										onClick={() => {
											handleReset();
											onCancel();
										}}
										type="button"
									>
										Cancel
									</button>
								)}

								<button
									className={getButtonClassName({ variant: "gray" })}
									disabled={!canSubmit || noCommits || isAnyPending || !hasChanges}
									type="submit"
								>
									{/* The reason rides in the label, not a tooltip: it is the
									    form's whole story, so it has to be readable without hover
									    (DESIGN.md → Empty states). */}
									{!isNew
										? "Save changes"
										: noCommits
											? "No commits yet"
											: pushFirst !== null
												? "Push and create a PR"
												: "Create a PR"}
									{/* Creating opens a PR; saving only confirms an edit. */}
									<Icon name={isAnyPending ? "spinner" : isNew ? "pr" : "tick"} />
								</button>
							</div>
						</div>
					</div>
				</div>
			</div>
		</form>
	);
};

/** A designed action whose backing feature does not exist yet. */
/**
 * The line under the title: who opened the review, how many commits it
 * carries and, once it has moved on from opening, when it last did. The side
 * panel keeps the opening time.
 */
export const PullRequestMeta: FC<{ projectId: string; review: ForgeReview }> = ({
	projectId,
	review,
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	// The review itself does not say how many commits it holds; the forge's
	// timeline, one event per commit currently on the review, does. It is the
	// same query the side panel's Activity section reads, so the count costs
	// nothing extra — and, like that section, only forges with a conversation
	// serve it.
	const { data: commitCount } = useQuery({
		...listReviewTimelineEventsQueryOptions({ projectId, reviewId: review.number }),
		enabled: forgeInfo?.capabilities.reviewComments !== false,
		select: (events) => events.filter((event) => event.kind === "committed").length,
	});
	const createdAtMs = review.createdAt === null ? null : Date.parse(review.createdAt);
	const modifiedAtMs = review.modifiedAt === null ? null : Date.parse(review.modifiedAt);
	// The forge stamps modified_at on any activity, so creation itself can
	// leave the two a moment apart; only a real gap is worth mentioning.
	const updated =
		modifiedAtMs !== null && (createdAtMs === null || modifiedAtMs - createdAtMs > 60_000)
			? modifiedAtMs
			: null;
	const commits = commitCount !== undefined && commitCount > 0 ? commitCount : null;
	if (review.author === null && commits === null && updated === null) return null;

	return (
		<div className={classes("text-13", styles.prViewMeta)}>
			{review.author !== null && <ReviewUser user={review.author} />}
			{commits !== null && (
				<span>
					{commits} commit{commits === 1 ? "" : "s"}
				</span>
			)}
			{updated !== null && (
				<span title={formatAbsoluteTime(updated)}>updated {formatRelativeTime(updated)}</span>
			)}
		</div>
	);
};

/** Rendered PR title and body; the header's Edit button flips to the form. */
export const PullRequestDescription: FC<{
	projectId: string;
	sourceBranch: string;
	reviewId: number;
	title: string;
	body: string | null;
	/** Sits between the title and the body while not editing. */
	meta?: ReactNode;
	canSubmit: boolean;
	editing: boolean;
	onDoneEditing: () => void;
	/** Absent where the review is read-only, so the empty body offers no edit. */
	onStartEditing?: () => void;
}> = ({
	projectId,
	sourceBranch,
	reviewId,
	title,
	body,
	meta,
	canSubmit,
	editing,
	onDoneEditing,
	onStartEditing,
}) => {
	const { data: reviewReactions } = useQuery({
		...listReviewReactionsQueryOptions({ projectId, reviewId }),
		select: tallyReactions,
	});
	const { data: currentLogin } = useQuery(currentForgeLoginQueryOptions(projectId));
	const { mutate: addReviewReaction } = useAddReviewReaction(projectId);
	const { mutate: removeReviewReaction } = useRemoveReviewReaction(projectId);
	const toggleReaction = (kind: string, myReactionId: number | null) => {
		if (myReactionId === null) addReviewReaction({ projectId, reviewId, kind });
		else removeReviewReaction({ projectId, reviewId, reactionId: myReactionId });
	};
	// "Add one" and the header's Edit button share one edit mode, but the
	// former was clicked to write a description, so the form opens on it.
	const [autofocus, setAutofocus] = useState<"title" | "body">("title");
	const doneEditing = () => {
		setAutofocus("title");
		onDoneEditing();
	};

	if (editing) {
		return (
			// Own boundary: the form suspends on its first idb draft read, and
			// without this the whole PR tab flashes to the tab-level fallback.
			<Suspense fallback={null}>
				<PullRequestForm
					body={body}
					projectId={projectId}
					reviewId={reviewId}
					sourceBranch={sourceBranch}
					title={title}
					canSubmit={canSubmit}
					autofocus={autofocus}
					onAfterSubmit={doneEditing}
					onCancel={doneEditing}
				/>
			</Suspense>
		);
	}

	return (
		<div className={styles.prView}>
			<h3 className={styles.prViewTitle}>{title}</h3>

			{meta}

			{body !== null && body.trim() !== "" ? (
				// A long description opens as a four-line card so the conversation
				// below is in reach: four rather than three because a body that opens
				// with a heading spends a line and a half on it. The slack means a
				// fold always hides at least a few lines, never just one.
				<Clamped maxHeight="4lh" foldOver="6lh" variant="card">
					<Markdown>{body}</Markdown>
				</Clamped>
			) : (
				<p className={classes("text-14", "text-body", styles.prViewEmptyBody)}>
					No description
					{onStartEditing !== undefined && (
						<>
							{" — "}
							<button
								className={styles.prViewAddDescription}
								type="button"
								onClick={() => {
									setAutofocus("body");
									onStartEditing();
								}}
							>
								Add one
							</button>
						</>
					)}
				</p>
			)}

			{reviewReactions !== undefined && (
				<Reactions
					reactions={reviewReactions.counts}
					reactors={reviewReactions.reactors}
					myLogin={currentLogin}
					onToggle={toggleReaction}
				/>
			)}
		</div>
	);
};

/** Why the Merge button is disabled, or null when merging is possible. */
const mergeBlockedReason = (mergeStatus: ReviewMergeStatus | undefined): string | null => {
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

/** The choice persists per project (see mergeMethodQueryOptions). */
const mergeMethods = [
	"merge",
	"squash",
	"rebase",
] as const satisfies ReadonlyArray<ReviewMergeMethod>;

const mergeMethodLabels: Record<ReviewMergeMethod, string> = {
	merge: "Merge",
	squash: "Squash and merge",
	rebase: "Rebase and merge",
};

/**
 * Edit / auto-merge / merge-with-method / overflow actions. Draft toggling
 * and closing live in the overflow menu.
 * Rendered in the details header row (next to the Diff|PR tab toggle),
 * only while the PR tab is showing.
 */
export const PullRequestPrimaryAction: FC<{
	projectId: string;
	review: ForgeReview;
	isEditing: boolean;
	onStartEdit: () => void;
}> = ({ projectId, review, isEditing, onStartEdit }) => {
	const { number: reviewId, draft: isDraft, autoMergeEnabled } = review;
	// A merged review is closed too, so merge wins when both timestamps are set.
	const isMerged = review.mergedAt !== null;
	const isClosed = !isMerged && review.closedAt !== null;

	const { data: mergeStatus } = useQuery({
		...getReviewMergeStatusQueryOptions({ projectId, reviewId }),
		// Minimise API calls.
		enabled: !isDraft,
	});
	const { data: storedMergeMethod } = useQuery(mergeMethodQueryOptions(projectId));
	const mergeMethod = storedMergeMethod ?? "merge";
	const { mutate: persistMergeMethod } = usePersistMergeMethod();

	const { isPending: isUpdateReviewPending, mutate: updateReview } = useUpdateReview(projectId);
	const { isPending: isMergeReviewPending, mutate: mergeReview } = useMergeReview(projectId);
	const { isPending: isSetReviewDraftinessPending, mutate: setReviewDraftiness } =
		useSetReviewDraftiness(projectId);
	const { isPending: isSetReviewAutoMergePending, mutate: setReviewAutoMerge } =
		useSetReviewAutoMerge(projectId);

	const isAnyPending =
		isUpdateReviewPending ||
		isMergeReviewPending ||
		isSetReviewDraftinessPending ||
		isSetReviewAutoMergePending;

	const blockedReason = mergeBlockedReason(mergeStatus);

	// A merged review can be neither drafted nor reopened, so its menu is the
	// browser link alone; `nativeMenuItemsFromGroups` would otherwise trail a
	// separator after an empty group.
	const stateItems = isMerged
		? []
		: [
				...(isClosed
					? []
					: [
							nativeMenuItem({
								label: isDraft ? "Mark as ready for review" : "Convert to draft",
								onSelect: () => setReviewDraftiness({ projectId, reviewId, draft: !isDraft }),
							}),
						]),
				nativeMenuItem({
					label: isClosed ? "Reopen pull request" : "Close pull request",
					onSelect: () =>
						updateReview({
							projectId,
							reviewId,
							state: isClosed ? "open" : "closed",
							title: null,
							body: null,
							targetBase: null,
						}),
				}),
			];

	const menuItems = nativeMenuItemsFromGroups(
		[
			[
				nativeMenuItem({
					label: "Open pull request in browser",
					onSelect: () => window.lite.openInWebBrowser(review.htmlUrl),
				}),
			],
			stateItems,
		].filter((group) => group.length > 0),
	);

	return (
		<div className={styles.prActions}>
			{/* One-way: the form's own Cancel and Save leave edit mode. */}
			<button
				className={getButtonClassName({ variant: "ghost" })}
				disabled={isAnyPending || isEditing}
				onClick={onStartEdit}
				type="button"
			>
				Edit
				<Icon name="edit" />
			</button>

			{!isDraft && (
				<>
					{/* Optimistic: the cache patch flips `checked` instantly, so no
					    spinner — but like its neighbors it locks while any of the
					    row's mutations are in flight. */}
					<SwitchButton
						label="Auto-merge"
						variant="outline"
						checked={autoMergeEnabled}
						disabled={isAnyPending}
						onCheckedChange={(enable) => setReviewAutoMerge({ projectId, reviewId, enable })}
					/>

					<DropdownButton
						variant="pop"
						disabled={isAnyPending || blockedReason !== null}
						onClick={() => mergeReview({ projectId, reviewId, mergeMethod })}
						actionTooltip={!isAnyPending && blockedReason !== null ? blockedReason : undefined}
						menuLabel="Merge method"
						menuDisabled={isAnyPending}
						onMenuTrigger={(trigger) =>
							void showNativeMenuFromTrigger(
								trigger,
								mergeMethods.map((method) =>
									nativeMenuItem({
										label: mergeMethodLabels[method],
										checked: method === mergeMethod,
										onSelect: () => persistMergeMethod({ projectId, method }),
									}),
								),
							)
						}
					>
						{isMergeReviewPending && <Icon name="spinner" />}
						{mergeMethodLabels[mergeMethod]}
					</DropdownButton>
				</>
			)}

			<button
				aria-label="More pull request actions"
				className={getButtonClassName({ variant: "ghost", iconOnly: true })}
				disabled={isAnyPending}
				onClick={(evt) => void showNativeMenuFromTrigger(evt.currentTarget, menuItems)}
				type="button"
			>
				{isUpdateReviewPending || isSetReviewDraftinessPending ? (
					<Icon name="spinner" />
				) : (
					<Icon name="kebab" />
				)}
			</button>
		</div>
	);
};
