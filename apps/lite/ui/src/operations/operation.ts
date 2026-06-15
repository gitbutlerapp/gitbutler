import { Toast } from "@base-ui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";
import {
	type CommitAmendParams,
	type CommitCreateParams,
	type CommitInsertBlankParams,
	type CommitMoveParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
	type MoveBranchParams,
	CommitSquashParams,
	CommitUncommitParams,
} from "#electron/ipc.ts";
import { headInfoQueryOptions, QueryKey } from "#ui/api/queries.ts";
import { rejectedChangesToastOptions } from "#ui/operations/toastOptions.tsx";
import { DiffSpec, InsertSide, RelativeTo } from "@gitbutler/but-sdk";
import { Operand, operandEquals, operandFileParent } from "#ui/operands.ts";
import { resolveDiffSpecs, useResolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import { decodeBytes } from "#ui/api/ref-name.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { useParams } from "@tanstack/react-router";
import { errorMessageForToast } from "#ui/errors.ts";

type CommitAmendOperation = Omit<CommitAmendParams, "dryRun" | "projectId" | "changes"> & {
	source: Operand;
};
type CommitCreateOperation = Omit<CommitCreateParams, "dryRun" | "projectId" | "changes"> & {
	source: Operand;
};
type CommitSplitOperation = Omit<CommitInsertBlankParams, "dryRun" | "projectId"> &
	Pick<CommitMoveChangesBetweenParams, "sourceCommitId"> & {
		source: Operand;
	};
type CommitMoveOperation = Omit<CommitMoveParams, "dryRun" | "projectId">;
type CommitMoveChangesBetweenOperation = Omit<
	CommitMoveChangesBetweenParams,
	"dryRun" | "projectId" | "changes"
> & { source: Operand };
type CommitSquashOperation = Omit<CommitSquashParams, "dryRun" | "projectId">;
type CommitUncommitOperation = Omit<CommitUncommitParams, "dryRun" | "projectId">;
type CommitUncommitChangesOperation = Omit<
	CommitUncommitChangesParams,
	"dryRun" | "projectId" | "changes"
> & { source: Operand };
type MoveBranchOperation = Omit<MoveBranchParams, "dryRun" | "projectId">;

export type Operation =
	| ({ _tag: "CommitAmend" } & CommitAmendOperation)
	| ({ _tag: "CommitCreate" } & CommitCreateOperation)
	| ({ _tag: "CommitSplit" } & CommitSplitOperation)
	| ({ _tag: "CommitMove" } & CommitMoveOperation)
	| ({ _tag: "CommitMoveChangesBetween" } & CommitMoveChangesBetweenOperation)
	| ({ _tag: "CommitSquash" } & CommitSquashOperation)
	| ({ _tag: "CommitUncommit" } & CommitUncommitOperation)
	| ({ _tag: "CommitUncommitChanges" } & CommitUncommitChangesOperation)
	| ({ _tag: "MoveBranch" } & MoveBranchOperation);

const commitAmendOperation = (operation: CommitAmendOperation): Operation => ({
	_tag: "CommitAmend",
	...operation,
});

const commitCreateOperation = (operation: CommitCreateOperation): Operation => ({
	_tag: "CommitCreate",
	...operation,
});

const commitSplitOperation = (operation: CommitSplitOperation): Operation => ({
	_tag: "CommitSplit",
	...operation,
});

const commitMoveOperation = (operation: CommitMoveOperation): Operation => ({
	_tag: "CommitMove",
	...operation,
});

const commitMoveChangesBetweenOperation = (
	operation: CommitMoveChangesBetweenOperation,
): Operation => ({
	_tag: "CommitMoveChangesBetween",
	...operation,
});

const commitSquashOperation = (operation: CommitSquashOperation): Operation => ({
	_tag: "CommitSquash",
	...operation,
});

const commitUncommitOperation = (operation: CommitUncommitOperation): Operation => ({
	_tag: "CommitUncommit",
	...operation,
});

const commitUncommitChangesOperation = (operation: CommitUncommitChangesOperation): Operation => ({
	_tag: "CommitUncommitChanges",
	...operation,
});

const moveBranchOperation = (operation: MoveBranchOperation): Operation => ({
	_tag: "MoveBranch",
	...operation,
});

export const operationLabel = (operation: Operation): string =>
	Match.value(operation).pipe(
		Match.tagsExhaustive({
			CommitAmend: () => "Amend",
			CommitCreate: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Commit above"),
					Match.when("below", () => "Commit below"),
					Match.exhaustive,
				),
			CommitSplit: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Commit above"),
					Match.when("below", () => "Commit below"),
					Match.exhaustive,
				),
			CommitMove: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Move above"),
					Match.when("below", () => "Move below"),
					Match.exhaustive,
				),
			CommitMoveChangesBetween: () => "Amend",
			CommitSquash: () => "Squash",
			CommitUncommit: () => "Uncommit",
			CommitUncommitChanges: () => "Uncommit",
			MoveBranch: () => "Move above",
		}),
	);

const runOperation = async ({
	projectId,
	operation,
	resolveChanges,
	dryRun,
}: {
	projectId: string;
	operation: Operation;
	resolveChanges: (source: Operand) => Promise<Array<DiffSpec> | null>;
	dryRun: boolean;
}) =>
	Match.value(operation).pipe(
		Match.tagsExhaustive({
			CommitAmend: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitAmend({
					projectId,
					commitId: operation.commitId,
					changes,
					dryRun,
				});
			},
			CommitMoveChangesBetween: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitMoveChangesBetween({
					projectId,
					sourceCommitId: operation.sourceCommitId,
					destinationCommitId: operation.destinationCommitId,
					changes,
					dryRun,
				});
			},
			CommitSquash: (operation) =>
				window.lite.commitSquash({
					projectId,
					sourceCommitIds: operation.sourceCommitIds,
					destinationCommitId: operation.destinationCommitId,
					dryRun,
				}),
			CommitUncommit: (operation) =>
				window.lite.commitUncommit({
					projectId,
					subjectCommitIds: operation.subjectCommitIds,
					assignTo: operation.assignTo,
					dryRun,
				}),
			CommitUncommitChanges: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitUncommitChanges({
					projectId,
					commitId: operation.commitId,
					assignTo: operation.assignTo,
					changes,
					dryRun,
				});
			},
			CommitCreate: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitCreate({
					projectId,
					relativeTo: operation.relativeTo,
					side: operation.side,
					changes,
					message: operation.message,
					dryRun,
				});
			},
			CommitSplit: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;

				// We can't dry run this as it's not an atomic operation. Ideally this
				// would be an atomic backend operation.
				if (dryRun) return null;

				const insertedCommit = await window.lite.commitInsertBlank({
					projectId,
					relativeTo: operation.relativeTo,
					side: operation.side,
					dryRun,
				});

				return window.lite.commitMoveChangesBetween({
					projectId,
					sourceCommitId:
						insertedCommit.workspace.replacedCommits[operation.sourceCommitId] ??
						operation.sourceCommitId,
					destinationCommitId: insertedCommit.newCommit,
					changes,
					dryRun,
				});
			},
			CommitMove: (operation) =>
				window.lite.commitMove({
					projectId,
					subjectCommitIds: operation.subjectCommitIds,
					relativeTo: operation.relativeTo,
					side: operation.side,
					dryRun,
				}),
			MoveBranch: (operation) =>
				window.lite.moveBranch({
					projectId,
					subjectBranch: operation.subjectBranch,
					targetBranch: operation.targetBranch,
					dryRun,
				}),
		}),
	);

export const useDryRunOperation = ({
	projectId,
	operation,
}: {
	projectId: string;
	operation?: Operation;
}) => {
	const changes = useResolveDiffSpecs({
		projectId,
		operand: operation && "source" in operation ? operation.source : undefined,
	});

	return useQuery({
		enabled: !!operation,
		queryKey: [QueryKey.DryRun, projectId, operation, changes],
		queryFn: () => {
			if (!operation) return null;
			return runOperation({
				projectId,
				operation,
				resolveChanges: async () => changes,
				dryRun: true,
			});
		},
		// We may have a lot of different dry runs in a short amount of time.
		gcTime: 10_000,
	});
};

export const useRunOperation = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (operation: Operation) =>
			runOperation({
				projectId,
				operation,
				resolveChanges: (source) => resolveDiffSpecs({ projectId, queryClient, source }),
				dryRun: false,
			}),
		onSuccess: async (response, _input, _ctx, { client }) => {
			if (response) {
				client.setQueryData(headInfoQueryOptions(projectId).queryKey, response.workspace.headInfo);
				dispatch(
					projectActions.updateRewrittenCommitReferences({
						projectId,
						replacedCommits: response.workspace.replacedCommits,
						headInfo: response.workspace.headInfo,
					}),
				);

				if ("rejectedChanges" in response && response.rejectedChanges.length > 0)
					toastManager.add(
						rejectedChangesToastOptions({
							newCommit: response.newCommit,
							rejectedChanges: response.rejectedChanges,
						}),
					);
			}
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to run operation",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

/**
 * | SOURCE ↓ / TARGET →    | Changes  | Commit |
 * | ---------------------- | -------- | ------ |
 * | File/hunk from changes | No-op    | Amend  |
 * | File/hunk from commit  | Uncommit | Amend  |
 * | Commit                 | Uncommit | Squash |
 */
const squashOperation = ({
	source,
	target,
}: {
	source: Operand;
	target: Operand;
}): Operation | null =>
	Match.value({ source, sourceFileParent: operandFileParent(source), target }).pipe(
		Match.when(
			{
				source: { _tag: "Commit" },
				target: { _tag: "Commit" },
			},
			({ source, target }) =>
				commitSquashOperation({
					sourceCommitIds: [source.commitId],
					destinationCommitId: target.commitId,
				}),
		),
		Match.when(
			{
				source: { _tag: "Commit" },
				target: { _tag: "ChangesSection" },
			},
			({ source }) =>
				commitUncommitOperation({
					subjectCommitIds: [source.commitId],
					assignTo: null,
				}),
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Changes" },
				target: { _tag: "Commit" },
			},
			({ source, target }) =>
				commitAmendOperation({
					commitId: target.commitId,
					source,
				}),
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Commit" },
				target: { _tag: "ChangesSection" },
			},
			({ source, sourceFileParent }) =>
				commitUncommitChangesOperation({
					commitId: sourceFileParent.commitId,
					assignTo: null,
					source,
				}),
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Commit" },
				target: { _tag: "Commit" },
			},
			({ source, sourceFileParent, target }) =>
				commitMoveChangesBetweenOperation({
					sourceCommitId: sourceFileParent.commitId,
					destinationCommitId: target.commitId,
					source,
				}),
		),
		Match.orElse(() => null),
	);

const moveOperation = ({
	source,
	target,
	side,
}: {
	source: Operand;
	target: Operand;
	side: InsertSide;
}) => {
	const branchMoveOperation = Match.value({ source, target, side }).pipe(
		// This should support `relativeTo`:
		// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
		// https://linear.app/gitbutler/issue/GB-1199/support-moving-branches-relative-to-commits
		// https://linear.app/gitbutler/issue/GB-1232/support-moving-branch-before-another-branch
		Match.when(
			{
				source: { _tag: "Branch" },
				target: { _tag: "Branch" },
				side: "above",
			},
			({ source, target }) =>
				moveBranchOperation({
					subjectBranch: decodeBytes(source.branchRef),
					targetBranch: decodeBytes(target.branchRef),
				}),
		),
		Match.orElse(() => null),
	);

	if (branchMoveOperation) return branchMoveOperation;

	const relativeTo: RelativeTo | null = Match.value({ target, side }).pipe(
		Match.when({ target: { _tag: "Commit" } }, ({ target }): RelativeTo | null => ({
			type: "commit",
			subject: target.commitId,
		})),
		Match.when(
			{
				target: { _tag: "Branch" },
				// We use the branch operand as the source/target for the branch
				// contents. However, `RelativeTo` is interpreted to mean just the
				// branch reference rather than the branch bucket, meaning `side:
				// "below"` won't work as expected.
				side: "above",
			},
			({ target }): RelativeTo | null => ({ type: "referenceBytes", subject: target.branchRef }),
		),
		Match.orElse((): RelativeTo | null => null),
	);

	if (!relativeTo) return null;

	return Match.value({ source, sourceFileParent: operandFileParent(source) }).pipe(
		Match.when({ source: { _tag: "Commit" } }, ({ source }) =>
			commitMoveOperation({
				subjectCommitIds: [source.commitId],
				relativeTo,
				side,
			}),
		),
		Match.when({ sourceFileParent: { _tag: "Changes" } }, ({ source }) =>
			commitCreateOperation({
				relativeTo,
				side,
				source,
				message: "",
			}),
		),
		Match.when({ sourceFileParent: { _tag: "Commit" } }, ({ source, sourceFileParent }) =>
			commitSplitOperation({
				sourceCommitId: sourceFileParent.commitId,
				relativeTo,
				side,
				source,
			}),
		),
		Match.orElse(() => null),
	);
};

export type OperationType = "squash" | "moveAbove" | "moveBelow";

const isOperationSourceEnabled = (source: Operand): boolean =>
	Match.value(source).pipe(
		Match.when({ _tag: "Hunk", isResultOfBinaryToTextConversion: true }, () => false),
		Match.orElse(() => true),
	);

export type OperationsByType = Record<OperationType, Operation | null>;

export const getOperations = (source: Operand, target: Operand): OperationsByType => {
	if (operandEquals(source, target) || !isOperationSourceEnabled(source))
		return {
			squash: null,
			moveAbove: null,
			moveBelow: null,
		};
	return {
		squash: squashOperation({ source, target }),
		moveAbove: moveOperation({ source, target, side: "above" }),
		moveBelow: moveOperation({ source, target, side: "below" }),
	};
};

export const getOperation = (x: {
	source: Operand;
	target: Operand;
	operationType: OperationType;
}): Operation | null => {
	const { squash, moveAbove, moveBelow } = getOperations(x.source, x.target);
	return Match.value(x.operationType).pipe(
		Match.when("squash", () => squash),
		Match.when("moveAbove", () => moveAbove),
		Match.when("moveBelow", () => moveBelow),
		Match.exhaustive,
	);
};
