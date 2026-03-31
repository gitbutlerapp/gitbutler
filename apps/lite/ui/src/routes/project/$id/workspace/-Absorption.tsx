import { absorbMutationOptions, absorptionPlanMutationOptions } from "#ui/api/mutations.ts";
import { commitTitle, shortCommitId } from "#ui/routes/project/$id/-shared.tsx";
import uiStyles from "#ui/ui.module.css";
import { AlertDialog, Toast } from "@base-ui/react";
import { AbsorptionReason, CommitAbsorption, TreeChange } from "@gitbutler/but-sdk";
import { useMutation } from "@tanstack/react-query";
import { FC, useState } from "react";
import styles from "./-Absorption.module.css";

const uniqueAbsorptionPaths = (commitAbsorption: CommitAbsorption): Array<string> =>
	globalThis.Array.from(new Set(commitAbsorption.files.map((file) => file.path)));

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

export const AbsorptionDialog: FC<{
	absorptionPlan: Array<CommitAbsorption>;
	isAbsorbing: boolean;
	onConfirm: () => void;
	onOpenChange: (open: boolean) => void;
}> = ({ absorptionPlan, isAbsorbing, onConfirm, onOpenChange }) => (
	<AlertDialog.Root open onOpenChange={onOpenChange}>
		<AlertDialog.Portal>
			<AlertDialog.Backdrop className={styles.backdrop} />
			<AlertDialog.Popup className={styles.popup}>
				<AlertDialog.Title>Absorb changes</AlertDialog.Title>
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
										{uniqueAbsorptionPaths(commitAbsorption).map((path) => (
											<li key={path}>{path}</li>
										))}
									</ul>
								</dd>
							</dl>
						</li>
					))}
				</ul>
				<div className={styles.actions}>
					<AlertDialog.Close className={uiStyles.button} disabled={isAbsorbing}>
						Cancel
					</AlertDialog.Close>
					<button
						type="button"
						className={uiStyles.button}
						onClick={onConfirm}
						disabled={absorptionPlan.length === 0 || isAbsorbing}
					>
						Absorb changes
					</button>
				</div>
			</AlertDialog.Popup>
		</AlertDialog.Portal>
	</AlertDialog.Root>
);

export const useAbsorption = (projectId: string) => {
	const [absorptionPlan, setAbsorptionPlan] = useState<Array<CommitAbsorption> | null>(null);
	const toastManager = Toast.useToastManager();

	const absorptionPlanMutation = useMutation(absorptionPlanMutationOptions);
	const absorbMutation = useMutation(absorbMutationOptions);

	const requestAbsorptionPlan = (changes: Array<TreeChange>, stackId: string | null) => {
		if (changes.length === 0) return;

		absorptionPlanMutation.mutate(
			{
				projectId,
				target: {
					type: "treeChanges",
					subject: {
						changes,
						assigned_stack_id: stackId,
					},
				},
			},
			{
				onSuccess: (plan) => {
					if (plan.length === 0) {
						toastManager.add({
							title: "No suitable commits found",
							description: "There are no commits available to absorb these changes into.",
						});
						return;
					}

					setAbsorptionPlan(plan);
				},
			},
		);
	};

	const confirmAbsorption = () => {
		if (absorptionPlan === null) return;

		absorbMutation.mutate(
			{
				projectId,
				absorptionPlan,
			},
			{
				onSuccess: () => {
					setAbsorptionPlan(null);
					toastManager.add({
						title: "Changes absorbed successfully",
					});
				},
			},
		);
	};

	return {
		absorptionPlan,
		isAbsorbing: absorbMutation.isPending,
		requestAbsorptionPlan,
		confirmAbsorption,
		clearAbsorptionPlan: () => {
			setAbsorptionPlan(null);
		},
	};
};
