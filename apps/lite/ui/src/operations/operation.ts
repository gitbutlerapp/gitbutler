import { Toast } from "@base-ui/react";
import { useMutation, useQuery, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
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
import { headInfoQueryOptions, type QueryKey } from "#ui/api/queries.ts";
import { rejectedChangesToastOptions } from "#ui/operations/toastOptions.tsx";
import { DiffSpec, InsertSide, RelativeTo } from "@gitbutler/but-sdk";
import { Operand, operandEquals, operandFileParent } from "#ui/operands.ts";
import { resolveDiffSpecs, useResolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { useParams } from "@tanstack/react-router";
import { errorMessageForToast } from "#ui/errors.ts";
import { syncCoreCaches } from "#ui/api/mutations.ts";
import { getHeadInfoIndex, type HeadInfoIndex } from "#ui/api/ref-info.ts";

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

type Operation =
	| ({ _tag: "CommitAmend" } & CommitAmendOperation)
	| ({ _tag: "CommitCreate" } & CommitCreateOperation)
	| ({ _tag: "CommitSplit" } & CommitSplitOperation)
	| ({ _tag: "CommitMove" } & CommitMoveOperation)
	| ({ _tag: "CommitMoveChangesBetween" } & CommitMoveChangesBetweenOperation)
	| ({ _tag: "CommitSquash" } & CommitSquashOperation)
	| ({ _tag: "CommitUncommit" } & CommitUncommitOperation)
	| ({ _tag: "CommitUncommitChanges" } & CommitUncommitChangesOperation)
	| ({ _tag: "MoveBranch" } & MoveBranchOperation);

type OperationWithLabel = { operation: Operation; label: string };

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
	headInfoIndex,
}: {
	projectId: string;
	operation?: Operation;
	headInfoIndex: HeadInfoIndex;
}) => {
	const changes = useResolveDiffSpecs({
		projectId,
		operand: operation && "source" in operation ? operation.source : undefined,
		headInfoIndex,
	});

	return useQuery({
		enabled: !!operation,
		queryKey: ["dryRun" satisfies QueryKey, projectId, operation, changes],
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
	const queryClient = useQueryClient();
	const toastManager = Toast.useToastManager();
	const { data: headInfoIndex } = useSuspenseQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	return useMutation({
		mutationFn: (operation: Operation) =>
			runOperation({
				projectId,
				operation,
				resolveChanges: (source) =>
					resolveDiffSpecs({ projectId, queryClient, source, headInfoIndex }),
				dryRun: false,
			}),
		onSuccess: async (response, _input, _ctx, { client }) => {
			if (response) {
				syncCoreCaches(client, projectId, response);

				if ("rejectedChanges" in response && response.rejectedChanges.length > 0) {
					toastManager.add(
						rejectedChangesToastOptions({
							newCommit: response.newCommit,
							rejectedChanges: response.rejectedChanges,
						}),
					);
				}
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
	sources,
	target,
	headInfoIndex,
}: {
	sources: Array<Operand>;
	target: Operand;
	headInfoIndex: HeadInfoIndex;
}): OperationWithLabel | null => {
	if (
		target._tag === "Commit" &&
		sources.length > 0 &&
		sources.every((source) => source._tag === "Commit")
	) {
		const destinationCommitId = headInfoIndex.commitContextById(target.changeId)?.commit.id;
		if (destinationCommitId === undefined) return null;

		const sourceCommitIds: Array<string> = [];
		for (const source of sources) {
			const subject = headInfoIndex.commitContextById(source.changeId)?.commit.id;
			if (subject === undefined) return null;

			sourceCommitIds.push(subject);
		}

		return {
			operation: commitSquashOperation({
				sourceCommitIds,
				destinationCommitId,
			}),
			label: "Squash",
		};
	}

	if (
		target._tag === "UncommittedChanges" &&
		sources.length > 0 &&
		sources.every((source) => source._tag === "Commit")
	) {
		return {
			operation: commitUncommitOperation({
				subjectCommitIds: sources.flatMap((source) => {
					const subject = headInfoIndex.commitContextById(source.changeId)?.commit.id;
					return subject !== undefined ? [subject] : [];
				}),
				assignTo: null,
			}),
			label: "Uncommit",
		};
	}

	const [source, ...rest] = sources;
	if (!source || rest.length > 0) return null;

	return Match.value({ source, sourceFileParent: operandFileParent(source), target }).pipe(
		Match.withReturnType<OperationWithLabel | null>(),
		Match.when(
			{
				sourceFileParent: { _tag: "UncommittedChanges" },
				target: { _tag: "Commit" },
			},
			({ source, target }) => {
				const commitId = headInfoIndex.commitContextById(target.changeId)?.commit.id;
				if (commitId === undefined) return null;

				return {
					operation: commitAmendOperation({
						commitId,
						source,
					}),
					label: "Amend",
				};
			},
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Commit" },
				target: { _tag: "UncommittedChanges" },
			},
			({ source, sourceFileParent }) => {
				const commitId = headInfoIndex.commitContextById(sourceFileParent.changeId)?.commit.id;
				if (commitId === undefined) return null;

				return {
					operation: commitUncommitChangesOperation({
						commitId,
						assignTo: null,
						source,
					}),
					label: "Uncommit",
				};
			},
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Commit" },
				target: { _tag: "Commit" },
			},
			({ source, sourceFileParent, target }) => {
				const sourceCommitId = headInfoIndex.commitContextById(sourceFileParent.changeId)?.commit
					.id;
				if (sourceCommitId === undefined) return null;
				const destinationCommitId = headInfoIndex.commitContextById(target.changeId)?.commit.id;
				if (destinationCommitId === undefined) return null;

				return {
					operation: commitMoveChangesBetweenOperation({
						sourceCommitId,
						destinationCommitId,
						source,
					}),
					label: "Amend",
				};
			},
		),
		Match.orElse(() => null),
	);
};

const intoOperation = ({
	sources,
	target,
	headInfoIndex,
}: {
	sources: Array<Operand>;
	target: Operand;
	headInfoIndex: HeadInfoIndex;
}): OperationWithLabel | null => {
	const squash = squashOperation({ sources, target, headInfoIndex });
	if (squash) return squash;

	if (
		target._tag === "Branch" &&
		sources.length > 0 &&
		sources.every((source) => source._tag === "Commit")
	) {
		return {
			operation: commitMoveOperation({
				subjectCommitIds: sources.flatMap((source) => {
					const subject = headInfoIndex.commitContextById(source.changeId)?.commit.id;
					return subject !== undefined ? [subject] : [];
				}),
				relativeTo: { type: "referenceBytes", subject: target.branchRef },
				side: "below",
			}),
			label: "Move here",
		};
	}

	const [source, ...rest] = sources;
	if (!source || rest.length > 0) return null;

	return Match.value({ source, sourceFileParent: operandFileParent(source), target }).pipe(
		Match.when(
			{
				sourceFileParent: { _tag: "UncommittedChanges" },
				target: { _tag: "Branch" },
			},
			({ source, target }): OperationWithLabel => ({
				operation: commitCreateOperation({
					relativeTo: { type: "referenceBytes", subject: target.branchRef },
					side: "below",
					source,
					message: "",
				}),
				label: "Commit here",
			}),
		),
		Match.orElse(() => null),
	);
};

// https://linear.app/gitbutler/issue/GB-1735/support-all-permutations-of-moving-branches-and-commits
const moveOperation = ({
	sources,
	target,
	side,
	headInfoIndex,
}: {
	sources: Array<Operand>;
	target: Operand;
	side: InsertSide;
	headInfoIndex: HeadInfoIndex;
}): OperationWithLabel | null => {
	const relativeTo: RelativeTo | null = Match.value({ target, side }).pipe(
		Match.withReturnType<RelativeTo | null>(),
		Match.when({ target: { _tag: "Commit" } }, ({ target }) => {
			const subject = headInfoIndex.commitContextById(target.changeId)?.commit.id;
			return subject !== undefined
				? {
						type: "commit",
						subject,
					}
				: null;
		}),
		Match.when(
			{
				target: { _tag: "Branch" },
				// We use the branch operand as the source/target for the branch
				// contents. However, `RelativeTo` is interpreted to mean just the
				// branch reference rather than the branch bucket, meaning `side:
				// "below"` won't work as expected.
				side: "above",
			},
			({ target }) => ({ type: "referenceBytes", subject: target.branchRef }),
		),
		Match.orElse(() => null),
	);

	if (relativeTo && sources.length > 0 && sources.every((source) => source._tag === "Commit")) {
		return {
			operation: commitMoveOperation({
				subjectCommitIds: sources.flatMap((source) => {
					const subject = headInfoIndex.commitContextById(source.changeId)?.commit.id;
					return subject !== undefined ? [subject] : [];
				}),
				relativeTo,
				side,
			}),
			label: Match.value(side).pipe(
				Match.when("above", () => "Move above"),
				Match.when("below", () => "Move below"),
				Match.exhaustive,
			),
		};
	}

	const [source, ...rest] = sources;
	if (!source || rest.length > 0) return null;

	const branchMoveOperation = Match.value({ source, target, side }).pipe(
		Match.when(
			{
				source: { _tag: "Branch" },
				target: { _tag: "Branch" },
				side: "above",
			},
			({ source, target }): OperationWithLabel => ({
				operation: moveBranchOperation({
					subjectBranch: decodeBytes(source.branchRef),
					targetBranch: decodeBytes(target.branchRef),
				}),
				label: "Move above",
			}),
		),
		Match.orElse(() => null),
	);

	if (branchMoveOperation) return branchMoveOperation;

	if (!relativeTo) return null;

	return Match.value({ source, sourceFileParent: operandFileParent(source) }).pipe(
		Match.withReturnType<OperationWithLabel | null>(),
		Match.when({ sourceFileParent: { _tag: "UncommittedChanges" } }, ({ source }) => ({
			operation: commitCreateOperation({
				relativeTo,
				side,
				source,
				message: "",
			}),
			label: Match.value(side).pipe(
				Match.when("above", () => "Commit above"),
				Match.when("below", () => "Commit below"),
				Match.exhaustive,
			),
		})),
		Match.when({ sourceFileParent: { _tag: "Commit" } }, ({ source, sourceFileParent }) => {
			const sourceCommitId = headInfoIndex.commitContextById(sourceFileParent.changeId)?.commit.id;
			return sourceCommitId !== undefined
				? {
						operation: commitSplitOperation({
							sourceCommitId,
							relativeTo,
							side,
							source,
						}),
						label: Match.value(side).pipe(
							Match.when("above", () => "Commit above"),
							Match.when("below", () => "Commit below"),
							Match.exhaustive,
						),
					}
				: null;
		}),
		Match.orElse(() => null),
	);
};

export type OperationType = "into" | "above" | "below";

const isOperationSourceEnabled = (source: Operand): boolean =>
	Match.value(source).pipe(
		Match.when({ _tag: "Hunk", isResultOfBinaryToTextConversion: true }, () => false),
		Match.orElse(() => true),
	);

export type OperationsByType = Record<OperationType, OperationWithLabel | null>;

export const getOperations = (
	sources: Array<Operand>,
	target: Operand,
	headInfoIndex: HeadInfoIndex,
): OperationsByType => {
	if (
		sources.length === 0 ||
		sources.some((source) => operandEquals(source, target)) ||
		!sources.every(isOperationSourceEnabled)
	) {
		return {
			into: null,
			above: null,
			below: null,
		};
	}
	return {
		into: intoOperation({ sources, target, headInfoIndex }),
		above: moveOperation({ sources, target, side: "above", headInfoIndex }),
		below: moveOperation({ sources, target, side: "below", headInfoIndex }),
	};
};

export const getOperation = (x: {
	sources: Array<Operand>;
	target: Operand;
	operationType: OperationType;
	headInfoIndex: HeadInfoIndex;
}): OperationWithLabel | null =>
	getOperations(x.sources, x.target, x.headInfoIndex)[x.operationType];
