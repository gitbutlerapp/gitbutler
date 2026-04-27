import { absorbMutationOptions } from "#ui/api/mutations.ts";
import { absorptionPlanQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import { commitTitle, shortCommitId } from "#ui/routes/project/$id/shared.tsx";
import uiStyles from "#ui/ui.module.css";
import { AlertDialog, Toast } from "@base-ui/react";
import {
	AbsorptionReason,
	AbsorptionTarget,
	HunkHeader,
	WorktreeChanges,
} from "@gitbutler/but-sdk";
import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { dedupe } from "effect/Array";
import { FC, Suspense } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { Item } from "./Item.ts";
import styles from "./Absorption.module.css";

const describeAbsorptionReason = (reason: AbsorptionReason): string | null => {
	switch (reason) {
		case "hunk_dependency":
			return "Files depend on this commit due to overlapping hunks.";
		case "stack_assignment":
			return "Files are assigned to this stack.";
		case "default_stack":
			return null;
	}
};

const hunkHeadersEqual = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart === b.oldStart &&
	a.oldLines === b.oldLines &&
	a.newStart === b.newStart &&
	a.newLines === b.newLines;

export const resolveAbsorptionTarget = ({
	item,
	worktreeChanges,
}: {
	item: Item;
	worktreeChanges: WorktreeChanges;
}): AbsorptionTarget | null =>
	Match.value(item).pipe(
		Match.withReturnType<AbsorptionTarget | null>(),
		Match.tag("ChangesSection", () => ({ type: "all" })),
		Match.when({ _tag: "File", parent: { _tag: "Changes" } }, ({ path }) => {
			const change = worktreeChanges.changes.find((candidate) => candidate.path === path);
			if (!change) return null;

			return {
				type: "treeChanges",
				subject: {
					changes: [change],
					assignedStackId: null,
				},
			};
		}),
		Match.when({ _tag: "Hunk", parent: { _tag: "Changes" } }, ({ path, hunkHeader }) => {
			const assignment = worktreeChanges.assignments.find(
				(candidate) =>
					candidate.path === path &&
					candidate.hunkHeader !== null &&
					hunkHeadersEqual(candidate.hunkHeader, hunkHeader),
			);
			if (!assignment) return null;

			return {
				type: "hunkAssignments",
				subject: {
					assignments: [assignment],
				},
			};
		}),
		Match.orElse(() => null),
	);

const AbsorptionDialogLoading: FC = () => (
	<>
		<div className={styles.body}>Loading absorption plan…</div>
		<div className={styles.actions}>
			<AlertDialog.Close className={uiStyles.button}>Cancel</AlertDialog.Close>
		</div>
	</>
);

const AbsorptionDialogError: FC = () => (
	<>
		<div className={styles.body}>There was a problem loading the absorption plan.</div>
		<div className={styles.actions}>
			<AlertDialog.Close className={uiStyles.button}>Cancel</AlertDialog.Close>
		</div>
	</>
);

const AbsorptionDialogContent: FC<{
	projectId: string;
	target: AbsorptionTarget;
	closeDialog: () => void;
}> = ({ projectId, target, closeDialog }) => {
	const toastManager = Toast.useToastManager();
	const { data: absorptionPlan } = useSuspenseQuery(
		absorptionPlanQueryOptions({ projectId, target }),
	);
	const absorbMutation = useMutation(absorbMutationOptions);

	const isEmpty = absorptionPlan.length === 0;

	const submitAction = () => {
		absorbMutation.mutate(
			{ projectId, absorptionPlan },
			{
				onSuccess: () => {
					closeDialog();
					toastManager.add({ title: "Changes absorbed successfully" });
				},
			},
		);
	};

	return (
		<form action={submitAction}>
			{isEmpty ? (
				<div className={styles.body}>
					There are no commits available to absorb these changes into.
				</div>
			) : (
				<ul className={styles.body}>
					{absorptionPlan.map((commitAbsorption) => (
						<li key={commitAbsorption.commitId}>
							<dl>
								<dt>Reason</dt>
								<dd>{describeAbsorptionReason(commitAbsorption.reason)}</dd>
								<dt>Commit message</dt>
								<dd>{commitTitle(commitAbsorption.commitSummary)}</dd>
								<dt>Commit ID</dt>
								<dd>
									<code>{shortCommitId(commitAbsorption.commitId)}</code>
								</dd>
								<dt>Paths</dt>
								<dd>
									<ul>
										{dedupe(commitAbsorption.files.map((file) => file.path)).map((path) => (
											<li key={path}>{path}</li>
										))}
									</ul>
								</dd>
							</dl>
						</li>
					))}
				</ul>
			)}
			<div className={styles.actions}>
				<AlertDialog.Close className={uiStyles.button}>Cancel</AlertDialog.Close>
				<button
					type="submit"
					className={uiStyles.button}
					disabled={isEmpty || absorbMutation.isPending}
				>
					Absorb changes
				</button>
			</div>
		</form>
	);
};

export const AbsorptionDialog: FC<{
	projectId: string;
	target: AbsorptionTarget;
	onOpenChange: (open: boolean) => void;
}> = ({ projectId, target, onOpenChange }) => (
	<AlertDialog.Root open onOpenChange={onOpenChange}>
		<AlertDialog.Portal>
			<AlertDialog.Backdrop className={uiStyles.dialogBackdrop} />
			<AlertDialog.Popup className={classes(uiStyles.popup, uiStyles.dialogPopup)}>
				<AlertDialog.Title>Absorb changes</AlertDialog.Title>
				<ErrorBoundary fallback={<AbsorptionDialogError />}>
					<Suspense fallback={<AbsorptionDialogLoading />}>
						<AbsorptionDialogContent
							projectId={projectId}
							target={target}
							closeDialog={() => onOpenChange(false)}
						/>
					</Suspense>
				</ErrorBoundary>
			</AlertDialog.Popup>
		</AlertDialog.Portal>
	</AlertDialog.Root>
);
