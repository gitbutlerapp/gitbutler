import {
	useAddReviewReaction,
	useGeneratePrDescription,
	useMergeReview,
	usePublishReview,
	useRemoveReviewReaction,
	useSetReviewAutoMerge,
	useSetReviewDraftiness,
	useUpdateReview,
} from "#ui/api/mutations.ts";
import {
	aiConfigurationQueryOptions,
	branchDetailsQueryOptions,
	currentForgeLoginQueryOptions,
	getReviewMergeStatusQueryOptions,
	listReviewReactionsQueryOptions,
} from "#ui/api/queries.ts";
import {
	groupReactors,
	Reactions,
	type ReactorsByKind,
} from "#ui/routes/project/$id/workspace/PullRequestReactions.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Clamped } from "#ui/components/Clamped.tsx";
import { classes } from "#ui/components/classes.ts";
import { DropdownButton } from "#ui/components/DropdownButton.tsx";
import { FieldControlStyles, FieldRootStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Markdown } from "#ui/components/Markdown.tsx";
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
import type {
	ForgeReview,
	ForgeReviewReaction,
	ForgeReviewReactionCount,
	ReviewMergeMethod,
	ReviewMergeStatus,
} from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useHotkey } from "@tanstack/react-hotkeys";
import { type FC, type SubmitEventHandler, Suspense, useEffect, useRef, useState } from "react";
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
	onAfterSubmit?: () => void;
	/** Adds a Cancel button that discards edits and calls this. */
	onCancel?: () => void;
	/**
	 * Called with the new PR's number right after it is created, for the
	 * settings the forge cannot take at creation time (labels, reviewers,
	 * auto-merge). Never called when editing an existing PR.
	 */
	afterPublish?: (reviewId: number) => void;
}> = ({
	projectId,
	sourceBranch,
	reviewId,
	title,
	body,
	canSubmit,
	onAfterSubmit,
	onCancel,
	afterPublish,
}) => {
	const { isPending: isPublishReviewPending, mutate: publishReview } = usePublishReview();
	const { isPending: isUpdateReviewPending, mutate: updateReview } = useUpdateReview();
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
	const isAnyPending = isPublishReviewPending || isUpdateReviewPending;
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

	const handleSubmit: SubmitEventHandler<HTMLFormElement> = (evt) => {
		evt.preventDefault();
		if (!canSubmit || isAnyPending || localDocument.title.trim() === "") return;

		if (reviewId === null) {
			publishReview(
				{
					projectId,
					params: {
						title: localDocument.title,
						body: localDocument.body,
						draft: localDocument.isDraft,
						localBranch: sourceBranch,
						sourceBranch,
					},
				},
				{ onSuccess: (outcome) => afterPublish?.(outcome.review.number) },
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
				{ onSuccess: () => onAfterSubmit?.() },
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
		<form ref={formRef} className={styles.prForm} onBlur={handleBlur} onSubmit={handleSubmit}>
			{/* Both fields name themselves in the placeholder, as designed, so
			    they carry an aria-label instead of a visible one. */}
			<Field.Root render={<FieldRootStyles />}>
				<Field.Control
					render={<FieldControlStyles />}
					aria-label="Pull request title"
					data-focus-scope={"pr" satisfies FocusScope}
					ref={useAutofocusScope()}
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
					ref={bodyRef}
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
									disabled={!canSubmit || isAnyPending || !hasChanges}
									type="submit"
								>
									{isNew ? "Create a PR" : "Save changes"}
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
/** Fold the raw reaction list into chip tallies plus who-reacted names. */
const reviewReactionsSelect = (
	reactions: Array<ForgeReviewReaction>,
): { counts: Array<ForgeReviewReactionCount>; reactors: ReactorsByKind } => {
	const tally = new Map<string, number>();
	for (const reaction of reactions) tally.set(reaction.kind, (tally.get(reaction.kind) ?? 0) + 1);
	return {
		counts: [...tally].map(([kind, count]) => ({ kind, count })),
		reactors: groupReactors(reactions),
	};
};

/** Rendered PR title and body; the header's Edit button flips to the form. */
export const PullRequestDescription: FC<{
	projectId: string;
	sourceBranch: string;
	reviewId: number;
	title: string;
	body: string | null;
	canSubmit: boolean;
	editing: boolean;
	onDoneEditing: () => void;
}> = ({ projectId, sourceBranch, reviewId, title, body, canSubmit, editing, onDoneEditing }) => {
	const { data: reviewReactions } = useQuery({
		...listReviewReactionsQueryOptions({ projectId, reviewId }),
		select: reviewReactionsSelect,
	});
	const { data: currentLogin } = useQuery(currentForgeLoginQueryOptions(projectId));
	const { mutate: addReviewReaction } = useAddReviewReaction();
	const { mutate: removeReviewReaction } = useRemoveReviewReaction();
	const toggleReaction = (kind: string, myReactionId: number | null) => {
		if (myReactionId === null) addReviewReaction({ projectId, reviewId, kind });
		else removeReviewReaction({ projectId, reviewId, reactionId: myReactionId });
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
					onAfterSubmit={onDoneEditing}
					onCancel={onDoneEditing}
				/>
			</Suspense>
		);
	}

	return (
		<div className={styles.prView}>
			<h3 className={classes("text-15", "text-semibold")}>{title}</h3>

			{body !== null && body.trim() !== "" ? (
				// Taller ceiling than comments: only truly huge descriptions fold.
				<Clamped maxHeight="80vh" skipWhenViewportFits>
					<Markdown>{body}</Markdown>
				</Clamped>
			) : (
				<p className={classes("text-13", styles.prViewEmptyBody)}>No description provided.</p>
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

	const { isPending: isUpdateReviewPending, mutate: updateReview } = useUpdateReview();
	const { isPending: isMergeReviewPending, mutate: mergeReview } = useMergeReview();
	const { isPending: isSetReviewDraftinessPending, mutate: setReviewDraftiness } =
		useSetReviewDraftiness();
	const { isPending: isSetReviewAutoMergePending, mutate: setReviewAutoMerge } =
		useSetReviewAutoMerge();

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
