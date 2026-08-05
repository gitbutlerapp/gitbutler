import {
	useAddReviewReaction,
	useMergeReview,
	usePublishReview,
	useRemoveReviewReaction,
	useSetReviewAutoMerge,
	useSetReviewDraftiness,
	useUpdateReview,
} from "#ui/api/mutations.ts";
import {
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
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { Clamped } from "#ui/components/Clamped.tsx";
import { classes } from "#ui/components/classes.ts";
import {
	FieldControlStyles,
	FieldLabelStyles,
	FieldRootStyles,
	FieldTextareaStyles,
} from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Markdown } from "#ui/components/Markdown.tsx";
import { Switch } from "#ui/components/Switch.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { pullRequestHotkeys } from "#ui/hotkeys.ts";
import { nativeMenuItem, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import {
	draftPRQueryOptions,
	mergeMethodQueryOptions,
	useDeleteDraftPR,
	usePersistDraftPR,
	usePersistMergeMethod,
} from "#ui/pr.ts";
import { type SelectionScope, useAutofocusSelectionScope } from "#ui/selection-scopes.ts";
import { Field, Tooltip } from "@base-ui/react";
import type {
	ForgeReviewReaction,
	ForgeReviewReactionCount,
	ReviewMergeMethod,
	ReviewMergeStatus,
} from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useHotkey } from "@tanstack/react-hotkeys";
import { type FC, type SubmitEventHandler, Suspense, useId, useRef, useState } from "react";
import styles from "./PullRequestTab.module.css";

/**
 * The building blocks of the Pull Request tab, defined bottom-up:
 *
 * - {@link PullRequestForm} — the create/edit form (also standalone for
 *   branches without a PR yet)
 * - {@link PullRequestDescription} — the rendered title/body view that
 *   flips into the form when edit mode is on
 * - {@link PullRequestPrimaryAction} — the Edit/draft/auto-merge/Merge
 *   button row; note it mounts in the details *header*, not the tab body
 *
 * `BranchDetails` (Details.tsx) owns the tab switching and edit-mode state
 * and composes these with `PullRequestPanel` and `PullRequestComments`.
 */

/**
 * Title/description form for creating a PR or editing an existing one.
 *
 * Unsubmitted input persists to idb per project+branch (survives restarts,
 * follows renames) and is cleared on Reset/Cancel. When the remote PR
 * changes underneath an *untouched* form, the fields follow the remote;
 * once locally edited, local wins until Reset. With `onCancel` set, Reset
 * becomes Cancel: discard edits and leave edit mode.
 */
export const PullRequestForm: FC<{
	projectId: string;
	sourceBranch: string;
	reviewId: number | null;
	title: string | null;
	body: string | null;
	canSubmit: boolean;
	onAfterSubmit?: () => void;
	/** Replaces Reset with a Cancel button that discards edits and calls this. */
	onCancel?: () => void;
}> = ({ projectId, sourceBranch, reviewId, title, body, canSubmit, onAfterSubmit, onCancel }) => {
	const { isPending: isPublishReviewPending, mutate: publishReview } = usePublishReview();
	const { isPending: isUpdateReviewPending, mutate: updateReview } = useUpdateReview();
	const formRef = useRef<HTMLFormElement | null>(null);

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
				draft: localDocument,
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

	const handleSubmit: SubmitEventHandler<HTMLFormElement> = (evt) => {
		evt.preventDefault();
		if (!canSubmit || isAnyPending || localDocument.title.trim() === "") return;

		if (reviewId === null) {
			publishReview({
				projectId,
				params: {
					title: localDocument.title,
					body: localDocument.body,
					draft: localDocument.isDraft,
					localBranch: sourceBranch,
					sourceBranch,
				},
			});
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
			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>Title</Field.Label>
				<Field.Control
					render={<FieldControlStyles />}
					className="text-15 text-semibold"
					data-selection-scope={"pr" satisfies SelectionScope}
					ref={useAutofocusSelectionScope()}
					name="title"
					onChange={(evt) => setLocalDocument({ ...localDocument, title: evt.currentTarget.value })}
					placeholder="Title"
					required
					value={localDocument.title}
				/>
			</Field.Root>

			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>Description</Field.Label>
				<Field.Control
					render={<FieldTextareaStyles />}
					className="text-14 text-body text-monospace"
					name="body"
					onChange={(evt) => setLocalDocument({ ...localDocument, body: evt.currentTarget.value })}
					placeholder="Description"
					value={localDocument.body}
				/>
			</Field.Root>

			{isNew && (
				<Field.Root render={<FieldRootStyles />}>
					<Field.Label render={<FieldLabelStyles />}>Draft</Field.Label>
					<Checkbox
						checked={localDocument.isDraft}
						name="isDraft"
						onCheckedChange={(isDraft) => setLocalDocument({ ...localDocument, isDraft })}
					/>
				</Field.Root>
			)}

			<div className={styles.prFormActions}>
				{onCancel !== undefined ? (
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
				) : (
					<button
						className={getButtonClassName({})}
						disabled={isAnyPending || !hasChanges}
						onClick={handleReset}
						type="button"
					>
						Reset
					</button>
				)}

				<button
					className={getButtonClassName({ variant: "pop" })}
					disabled={!canSubmit || isAnyPending || !hasChanges}
					type="submit"
				>
					{isAnyPending && <Icon name="spinner" />}
					{isNew ? "Submit" : "Update"}
				</button>
			</div>
		</form>
	);
};

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
 * Edit / draft-toggle / auto-merge / merge-with-method / overflow actions.
 * Rendered in the details header row (next to the Diff|PR tab toggle),
 * only while the PR tab is showing.
 */
export const PullRequestPrimaryAction: FC<{
	projectId: string;
	reviewId: number;
	isDraft: boolean;
	autoMergeEnabled: boolean;
	isEditing: boolean;
	onToggleEdit: () => void;
}> = ({ projectId, reviewId, isDraft, autoMergeEnabled, isEditing, onToggleEdit }) => {
	const autoMergeSwitchId = useId();
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

	return (
		<div className={styles.prActions}>
			<button
				aria-pressed={isEditing}
				className={getButtonClassName({ variant: isEditing ? "gray" : "outline" })}
				disabled={isAnyPending}
				onClick={onToggleEdit}
				type="button"
			>
				<Icon name="edit" />
				Edit
			</button>

			<button
				className={getButtonClassName({ variant: !isDraft ? "outline" : "pop" })}
				disabled={isAnyPending}
				onClick={() => setReviewDraftiness({ projectId, reviewId, draft: !isDraft })}
				type="button"
			>
				{isSetReviewDraftinessPending && <Icon name="spinner" />}
				{isDraft ? "Mark as Ready" : "Convert to draft"}
			</button>

			{!isDraft && (
				<>
					<span className={classes("text-13", styles.autoMerge)}>
						{/* Optimistic: the cache patch flips `checked` instantly, so no
						    spinner — but like its neighbors it locks while any of the
						    row's mutations are in flight. */}
						<Switch
							id={autoMergeSwitchId}
							checked={autoMergeEnabled}
							disabled={isAnyPending}
							onCheckedChange={(enable) => setReviewAutoMerge({ projectId, reviewId, enable })}
						/>
						{/* A wrapping label would forward clicks back to the switch
						    button and double-toggle; the for-association doesn't. */}
						<label htmlFor={autoMergeSwitchId}>Auto-merge</label>
					</span>

					<div className={styles.splitButton}>
						{/* The trigger span always wraps the button so its tree position
						    is stable — a conditional wrapper would remount the button
						    (dropping focus) whenever blockedReason flips. */}
						<Tooltip.Root>
							{/* Disabled buttons swallow hover, so the wrapper span carries the tooltip. */}
							<Tooltip.Trigger render={<span className={styles.disabledActionWrap} />}>
								<button
									className={getButtonClassName({ variant: "pop" })}
									disabled={isAnyPending || blockedReason !== null}
									onClick={() => mergeReview({ projectId, reviewId, mergeMethod })}
									type="button"
								>
									{isMergeReviewPending && <Icon name="spinner" />}
									{mergeMethodLabels[mergeMethod]}
								</button>
							</Tooltip.Trigger>
							{!isAnyPending && blockedReason !== null && (
								<Tooltip.Portal>
									<Tooltip.Positioner sideOffset={4}>
										<Tooltip.Popup render={<TooltipPopup />}>{blockedReason}</Tooltip.Popup>
									</Tooltip.Positioner>
								</Tooltip.Portal>
							)}
						</Tooltip.Root>

						<button
							aria-label="Merge method"
							className={getButtonClassName({ variant: "pop", iconOnly: true })}
							disabled={isAnyPending}
							onClick={(evt) =>
								void showNativeMenuFromTrigger(
									evt.currentTarget,
									mergeMethods.map((method) =>
										nativeMenuItem({
											label: mergeMethodLabels[method],
											checked: method === mergeMethod,
											onSelect: () => persistMergeMethod({ projectId, method }),
										}),
									),
								)
							}
							type="button"
						>
							<Icon name="chevron-down" />
						</button>
					</div>
				</>
			)}

			<button
				aria-label="More pull request actions"
				className={styles.moreActions}
				disabled={isAnyPending}
				onClick={(evt) =>
					void showNativeMenuFromTrigger(evt.currentTarget, [
						nativeMenuItem({
							label: "Close pull request",
							onSelect: () =>
								updateReview({
									projectId,
									reviewId,
									state: "closed",
									title: null,
									body: null,
									targetBase: null,
								}),
						}),
					])
				}
				type="button"
			>
				{isUpdateReviewPending ? <Icon name="spinner" /> : <Icon name="kebab" />}
			</button>
		</div>
	);
};
