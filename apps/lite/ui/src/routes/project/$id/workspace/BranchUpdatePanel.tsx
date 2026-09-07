import rowStyles from "./Row.module.css";
import uiStyles from "#ui/components/ui.module.css";
import {
	applyPlanEdits,
	buildPreviewRows,
	integrationPlanQueryOptions,
	integrationPreviewQueryOptions,
	stepCommitIds,
	type PlanEdit,
	type PreviewRow,
} from "#ui/branch-integration.ts";
import {
	useApplyBranchIntegration,
	useWorkspaceBranchAndAncestorsPush,
} from "#ui/api/mutations.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { classes } from "#ui/components/classes.ts";
import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { authorTooltip, commitIsDiverged, commitTitle, shortCommitId } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import type { BranchIntegrationStrategy, FullRefName } from "@gitbutler/but-sdk";
import { Dialog, Toggle, ToggleGroup } from "@base-ui/react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { type FC, useState } from "react";
import { Row, RowCheckbox, RowLabel, RowLabelContainer } from "./Row.tsx";
import styles from "./BranchUpdatePanel.module.css";

/** Whose version wins where the two sides differ, or both sides kept. */
type Choice = "mine" | "combine" | "theirs";

const pluralRules = new Intl.PluralRules("en");
const count = (n: number, noun: string): string =>
	`${n} ${noun}${pluralRules.select(n) === "one" ? "" : "s"}`;

const shortRefName = (refName: FullRefName): string => {
	const full = refName.full;
	if (full.startsWith("refs/heads/")) return full.slice("refs/heads/".length);
	if (full.startsWith("refs/remotes/")) return full.slice("refs/remotes/".length);
	return full;
};

/** A leg's head: the ref's name where its rail begins. */
const LegHeadRow: FC<{
	label: string;
	status: GraphSegmentStatus;
}> = ({ label, status }) => (
	<Row interactive={false}>
		<GraphSegment glyph="forkRight" status={status} />
		<RowLabelContainer>
			<RowLabel heading singleLine title={label} className={rowStyles.fadedText}>
				{label}
			</RowLabel>
		</RowLabelContainer>
	</Row>
);

/** A commit unticked, held on screen so it can be ticked back. */
type DroppedRow = { commitId: string; subject: string };

/**
 * The previewed result: each commit coloured by its side, flagged where the
 * preview reports a conflict.
 */
const AfterOutline: FC<{
	branchLabel: string;
	rows: Array<PreviewRow>;
	dropped: Array<DroppedRow>;
	/** Pick: an incoming row unticked leaves the plan, and comes back ticked below. */
	pickable: boolean;
	onDrop: (row: PreviewRow) => void;
	onRestore: (commitId: string) => void;
}> = ({ branchLabel, rows, dropped, pickable, onDrop, onRestore }) => (
	<div>
		<LegHeadRow label={branchLabel} status="LocalOnly" />
		{rows.length === 0 && (
			<Row interactive={false}>
				<GraphSegment glyph="parent" status="LocalOnly" />
				<RowLabelContainer>
					<RowLabel className={rowStyles.fadedText}>No commits.</RowLabel>
				</RowLabelContainer>
			</Row>
		)}
		{rows.map((row) => {
			const { commit, origin } = row;
			return (
				<Row
					key={commit.id}
					interactive={false}
					title={authorTooltip(commit.author, commit.authoredAt)}
				>
					<GraphSegment
						glyph="commit"
						status={
							origin === "incoming"
								? "Upstream"
								: origin === "local"
									? "LocalOnly"
									: "LocalAndRemote"
						}
					/>
					{/* Their additions are each a choice; nothing else here is. */}
					{pickable && origin === "incoming" && (
						<RowCheckbox
							checked
							nativeButton
							aria-label={`Keep ${commitTitle(commit.message) ?? "(no message)"}`}
							onCheckedChange={() => onDrop(row)}
						/>
					)}
					<RowLabelContainer>
						{commit.hasConflicts && (
							<ConflictIcon
								variant="conflict"
								className={styles.conflictIcon}
								aria-label="Conflicted"
							/>
						)}
						<RowLabel singleLine>{commitTitle(commit.message) ?? "(no message)"}</RowLabel>
					</RowLabelContainer>
				</Row>
			);
		})}

		{dropped.length > 0 && (
			<div className={styles.dropped}>
				<p className={classes("text-12", rowStyles.fadedText, styles.droppedHeading)}>Left out</p>
				{dropped.map(({ commitId, subject }) => (
					<Row key={commitId} interactive={false}>
						<GraphSegment glyph="space" status="LocalOnly" />
						<RowCheckbox
							checked={false}
							nativeButton
							aria-label={`Keep ${subject}`}
							onCheckedChange={() => onRestore(commitId)}
						/>
						<RowLabelContainer>
							<RowLabel singleLine className={classes(rowStyles.fadedText, styles.droppedLabel)}>
								{subject}
							</RowLabel>
						</RowLabelContainer>
					</Row>
				))}
			</div>
		)}
	</div>
);

/**
 * The update-from-remote flow, led by a diagnosis: the dialog states what the
 * remote has and offers three answers, Keep mine, Combine both, Take theirs,
 * over one outline of the previewed outcome.
 */
const BranchUpdatePanel: FC<{
	projectId: string;
	/** The applied branch's full ref name. */
	branch: string;
	onApplied: () => void;
}> = ({ projectId, branch, onApplied }) => {
	/** `null` until the user picks: the default depends on what the remote has. */
	const [pickedChoice, setPickedChoice] = useState<Choice | null>(null);
	// The commits unticked, in pre-rewrite ids so they survive the preview
	// changing under them; picking a choice clears them.
	const [edits, setEdits] = useState<Array<PlanEdit>>([]);

	const pickChoice = (next: Choice) => {
		setPickedChoice(next);
		setEdits([]);
	};

	// Remote versions of rewritten local commits, as head info names them; the
	// plan's change ids cover the rest below.
	const { data: divergedSubjects } = useQuery({
		...headInfoQueryOptions(projectId),
		select: (headInfo) => {
			for (const stack of headInfo.stacks) {
				for (const segment of stack.segments) {
					if (segment.refName === null || decodeBytes(segment.refName.fullNameBytes) !== branch)
						continue;
					return segment.commits.flatMap((commit) =>
						commitIsDiverged(commit) && commit.state.type === "LocalAndRemote"
							? [commit.state.subject]
							: [],
					);
				}
			}
			return [];
		},
	});

	// Keep mine and Combine both share the rebase plan and differ only in
	// what gets dropped from it; Take theirs replaces outright.
	const strategy: BranchIntegrationStrategy =
		pickedChoice === "theirs" ? "pickRemote" : "pullRebase";
	const {
		data: plan,
		isError: isPlanError,
		isPlaceholderData: isPlanStale,
	} = useQuery({
		...integrationPlanQueryOptions({ projectId, branch, strategy }),
		// The previous plan stays on screen, dimmed, while a strategy switch refetches.
		placeholderData: keepPreviousData,
	});
	// What the remote has: twins of rewritten local commits, matched by change
	// id and by head info, and genuinely new commits.
	const localChangeIds = new Set(
		(plan?.divergence.localOnly ?? []).flatMap((commit) =>
			commit.changeId === null ? [] : [commit.changeId],
		),
	);
	const twinIds = new Set([
		...(plan?.divergence.upstreamOnly ?? []).flatMap((commit) =>
			commit.changeId !== null && localChangeIds.has(commit.changeId) ? [commit.id] : [],
		),
		...(divergedSubjects ?? []),
	]);
	const upstreamOnlyIds = (plan?.divergence.upstreamOnly ?? []).map((commit) => commit.id);
	const additions = upstreamOnlyIds.filter((id) => !twinIds.has(id)).length;
	const rewritten = upstreamOnlyIds.filter((id) => twinIds.has(id)).length;
	// With nothing new to take, keeping mine is the default.
	const choice: Choice = pickedChoice ?? (additions > 0 ? "combine" : "mine");

	// Keep mine leaves every remote commit out; Combine both leaves out only
	// the remote's versions of rewritten commits.
	const baselineEdits: Array<PlanEdit> =
		choice === "mine"
			? upstreamOnlyIds.map((commitId) => ({ commitId }))
			: choice === "combine"
				? [...twinIds].map((commitId) => ({ commitId }))
				: [];
	const allEdits = [...baselineEdits, ...edits];
	const integration =
		plan === undefined
			? undefined
			: allEdits.length === 0
				? plan.integration
				: { ...plan.integration, steps: applyPlanEdits(plan.integration.steps, allEdits) };
	const {
		data: previewResult,
		isError: isPreviewError,
		isPlaceholderData: isPreviewStale,
	} = useQuery({
		...integrationPreviewQueryOptions({ projectId, branch, integration }),
		placeholderData: keepPreviousData,
	});

	const previewRows =
		previewResult != null && plan !== undefined
			? buildPreviewRows({
					workspace: previewResult.workspace,
					branch,
					divergence: plan.divergence,
				})
			: null;

	const { isPending: isApplying, mutate: applyIntegration } = useApplyBranchIntegration();
	const {
		isPending: isPushing,
		mutate: pushBranch,
		error: pushError,
	} = useWorkspaceBranchAndAncestorsPush(projectId);
	// Done state: offers the push the integration now needs, rather than leaving it to the sidebar.
	const [applied, setApplied] = useState<{
		/** What landed: an integration still to publish, a push, or a replace. */
		kind: "integrated" | "pushed" | "replaced";
		force: boolean;
		remoteName: string;
	} | null>(null);

	if (isPlanError) {
		return (
			<p className={classes("text-13", styles.message)}>
				Could not compute the update. The upstream may have moved; try again after a fetch.
			</p>
		);
	}
	if (plan === undefined)
		return <p className={classes("text-13", styles.message)}>Computing the update…</p>;

	const localTotal = plan.divergence.localOnly.length;
	const incomingCount = previewRows?.filter((row) => row.origin === "incoming").length ?? null;
	const keptCount = previewRows?.filter((row) => row.origin === "local").length ?? null;
	const conflictCount = previewRows?.filter((row) => row.commit.hasConflicts).length ?? 0;
	const branchLabel = shortRefName(plan.divergence.branchRefName);
	const remoteName = shortRefName(plan.divergence.upstreamRefName);

	const allDivergenceCommits = [
		...plan.divergence.localOnly,
		...plan.divergence.upstreamOnly,
		plan.divergence.mergeBase,
	];
	// Only drops the current plan still holds: one orphaned by a refetched plan is not a removal.
	const planIds = new Set(plan.integration.steps.flatMap(stepCommitIds));
	const dropped = edits.flatMap((edit) =>
		planIds.has(edit.commitId)
			? [
					{
						commitId: edit.commitId,
						subject:
							allDivergenceCommits.find((commit) => commit.id === edit.commitId)?.subject ??
							shortCommitId(edit.commitId),
					},
				]
			: [],
	);
	const restore = (commitId: string) =>
		setEdits((current) => current.filter((edit) => edit.commitId !== commitId));
	// Once each: a drop landing twice would count twice in Reset.
	const dropRow = (row: PreviewRow) =>
		setEdits((current) => [
			...current,
			...row.tracedIds
				.filter((commitId) => !current.some((edit) => edit.commitId === commitId))
				.map((commitId) => ({ commitId })),
		]);
	const upstreamIds = new Set(plan.divergence.upstreamOnly.map((commit) => commit.id));
	const leftOutIncoming = dropped.filter((entry) => upstreamIds.has(entry.commitId)).length;

	// Combining rewrites the branch; a force push is needed only while the
	// remote still holds commits the result lacks.
	const needsForce = choice === "combine" && (rewritten > 0 || leftOutIncoming > 0);
	const apply = () => {
		if (choice === "mine") {
			// Keeping mine needs no integration: the remote just has to catch up.
			pushBranch(
				{
					projectId,
					branch,
					withForce: true,
					skipForcePushProtection: false,
					runHooks: true,
					pushOpts: [],
				},
				{ onSuccess: () => setApplied({ kind: "pushed", force: true, remoteName }) },
			);
			return;
		}
		if (integration === undefined) return;
		applyIntegration(
			{ projectId, branch, integration, dryRun: false },
			{
				// Taking theirs leaves nothing to push; combining offers the push in place.
				onSuccess: () =>
					setApplied(
						choice === "theirs"
							? { kind: "replaced", force: false, remoteName }
							: { kind: "integrated", force: needsForce, remoteName },
					),
			},
		);
	};
	const pushApplied = () => {
		if (applied === null) return;
		pushBranch(
			{
				projectId,
				branch,
				withForce: applied.force,
				skipForcePushProtection: false,
				runHooks: true,
				pushOpts: [],
			},
			{ onSuccess: () => setApplied({ ...applied, kind: "pushed" }) },
		);
	};

	if (applied !== null) {
		return (
			<div className={styles.panel}>
				<div className={styles.appliedBody}>
					<p className={classes("text-13", "text-semibold")}>
						{applied.kind === "integrated"
							? "Branch updated."
							: applied.kind === "pushed"
								? "Branch published."
								: "Branch replaced."}
					</p>
					<p className={classes("text-13", rowStyles.fadedText)}>
						{applied.kind === "integrated"
							? applied.force
								? `${applied.remoteName} still holds the previous versions of your commits; force push to publish the result.`
								: `${applied.remoteName} is behind; push to publish the result.`
							: applied.kind === "pushed"
								? `${applied.remoteName} now matches this branch.`
								: `This branch now matches ${applied.remoteName}.`}
					</p>
				</div>
				<div className={styles.footer}>
					{pushError !== null && (
						<span className={classes("text-12", styles.conflictNote)}>
							<ConflictIcon variant="conflict" aria-hidden />
							{errorMessageForToast(pushError)}
						</span>
					)}
					<button
						type="button"
						className={getButtonClassName({
							variant: applied.kind === "integrated" ? "outline" : "pop",
						})}
						onClick={onApplied}
					>
						Done
					</button>
					{applied.kind === "integrated" && (
						<button
							type="button"
							className={getButtonClassName({ variant: "pop" })}
							disabled={isPushing}
							onClick={pushApplied}
						>
							{isPushing && <Icon name="spinner" />}
							{applied.force ? "Force push" : "Push"}
						</button>
					)}
				</div>
			</div>
		);
	}

	return (
		<div className={styles.panel}>
			<div className={styles.controls}>
				<div className={styles.strategyRow}>
					<p className={classes("text-13", styles.diagnosis)}>
						{additions > 0 && rewritten > 0
							? `${remoteName} has ${count(additions, "new commit")}, and a different version of ${rewritten === 1 ? "one" : rewritten} of yours.`
							: additions > 0
								? `${remoteName} has ${count(additions, "new commit")}.`
								: rewritten > 0
									? `${remoteName} holds a different version of ${rewritten === 1 ? "one" : rewritten} of your commits.`
									: `${remoteName} and this branch have diverged.`}
					</p>
				</div>

				<ToggleGroup
					render={<ToggleGroupStyles />}
					value={[choice]}
					onValueChange={(value: Array<Choice>) => {
						const head = value[0];
						if (head !== undefined) pickChoice(head);
					}}
					aria-label="How to resolve the divergence"
				>
					<Toggle render={<ToggleStyles />} value="mine">
						Keep mine
					</Toggle>
					{/* Always in the row, so the three answers read as one set. */}
					<Toggle render={<ToggleStyles />} value="combine" disabled={additions === 0}>
						Combine both
					</Toggle>
					<Toggle render={<ToggleStyles />} value="theirs">
						Take theirs
					</Toggle>
				</ToggleGroup>

				<div className={styles.viewRow}>
					{edits.length > 0 && (
						<button
							type="button"
							className={getButtonClassName({ variant: "outline", size: "small" })}
							onClick={() => setEdits([])}
						>
							Reset {count(edits.length, "edit")}
						</button>
					)}

					{incomingCount !== null && keptCount !== null && (
						<span className={classes("text-12", rowStyles.fadedText, styles.summary)}>
							{leftOutIncoming > 0
								? `${incomingCount} of ${count(incomingCount + leftOutIncoming, "incoming commit")}`
								: count(incomingCount, "incoming commit")}{" "}
							· {keptCount} of {count(localTotal, "local commit")} kept
						</span>
					)}
				</div>
			</div>

			{/* Only the outcome: the sidebar already shows the current state. */}
			<div
				className={classes(uiStyles.scroller, styles.outline)}
				data-stale={isPreviewStale || isPlanStale ? true : undefined}
			>
				{isPreviewError ? (
					<p className={classes("text-13", styles.message)}>Could not preview this plan.</p>
				) : previewRows === null ? (
					<p className={classes("text-13", styles.message)}>Computing the preview…</p>
				) : (
					<AfterOutline
						branchLabel={branchLabel}
						rows={previewRows}
						dropped={dropped}
						pickable={choice === "combine"}
						onDrop={dropRow}
						onRestore={restore}
					/>
				)}
			</div>

			<div className={styles.footer}>
				<div className={styles.footerNotes}>
					{conflictCount > 0 && (
						<span className={classes("text-12", styles.conflictNote)}>
							<ConflictIcon variant="conflict" aria-hidden />
							{count(conflictCount, "commit")} will need conflict resolution.
						</span>
					)}
					{pushError !== null && (
						<span className={classes("text-12", styles.conflictNote)}>
							<ConflictIcon variant="conflict" aria-hidden />
							{errorMessageForToast(pushError)}
						</span>
					)}
					{choice === "combine" && (
						<span className={classes("text-12", rowStyles.fadedText)}>
							{needsForce
								? "Rewrites the branch; force push afterwards to publish."
								: "Push afterwards to publish."}
						</span>
					)}
				</div>
				<button
					type="button"
					className={getButtonClassName({ variant: "pop" })}
					// Held while either query still serves the previous key's data: applying then
					// would run a plan the outline is about to replace.
					disabled={
						choice === "mine"
							? isPushing
							: isApplying ||
								isPreviewError ||
								previewRows === null ||
								isPlanStale ||
								isPreviewStale
					}
					onClick={apply}
				>
					{(isApplying || isPushing) && <Icon name="spinner" />}
					{choice === "mine" ? "Force push" : choice === "theirs" ? "Replace branch" : "Integrate"}
				</button>
			</div>
		</div>
	);
};

/**
 * The flow in a modal. Mounted only while open, so every open starts fresh
 * on the recommended strategy with no edits.
 */
export const BranchUpdateDialog: FC<{
	projectId: string;
	branchRef: string;
	open: boolean;
	onOpenChange: (open: boolean) => void;
}> = ({ projectId, branchRef, open, onOpenChange }) => (
	<Dialog.Root open={open} onOpenChange={onOpenChange}>
		<Dialog.Portal>
			<Dialog.Backdrop className={styles.backdrop} />
			<Dialog.Viewport className={styles.viewport}>
				<Dialog.Popup aria-labelledby="branch-update-heading" className={styles.popup}>
					<div className={styles.dialogHeader}>
						<Icon name="branch" />
						<h1 id="branch-update-heading" className={classes("text-14", "text-bold")}>
							Update {shortRefName({ full: branchRef })} from remote
						</h1>
					</div>
					<BranchUpdatePanel
						projectId={projectId}
						branch={branchRef}
						onApplied={() => onOpenChange(false)}
					/>
				</Dialog.Popup>
			</Dialog.Viewport>
		</Dialog.Portal>
	</Dialog.Root>
);
