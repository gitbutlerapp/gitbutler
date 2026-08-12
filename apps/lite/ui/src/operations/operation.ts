/**
 * @file Plan, dry run, and execute operations upon potentially multiple sources and a target.
 *
 * Operations are declarative representations of mutations that may be performed, organised by
 * positional "placements".
 *
 * Executions upon operations may be previewed in terms of a dry run.
 */

import { Toast } from "@base-ui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";
import { rejectedChangesToastOptions } from "#ui/operations/toastOptions.tsx";
import type { DiffSpec, InsertSide, RelativeTo } from "@gitbutler/but-sdk";
import { type Operand, operandEquals, operandFileParent } from "#ui/operands.ts";
import { resolveDiffSpecs, useResolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { useAppDispatch } from "#ui/store.ts";
import { useParams } from "@tanstack/react-router";
import { errorMessageForToast } from "#ui/errors.ts";
import { syncCoreCaches } from "#ui/api/mutations.ts";

/**
 * Each operation type corresponds to a member of the SDK's `OperationKind` type and associated
 * single mutation. `SplitCommit` is our one exception as the SDK doesn't currently support that as
 * a single, atomic operation.
 */
type Operation =
	| { _tag: "AmendCommit"; sources: Array<Operand>; commitId: string }
	| {
			_tag: "CreateCommit";
			sources: Array<Operand>;
			relativeTo: RelativeTo;
			side: InsertSide;
			message: string;
	  }
	| {
			_tag: "SplitCommit";
			sources: Array<Operand>;
			sourceCommitId: string;
			relativeTo: RelativeTo;
			side: InsertSide;
	  }
	| {
			_tag: "MoveCommit";
			subjectCommitIds: Array<string>;
			relativeTo: RelativeTo;
			side: InsertSide;
	  }
	| {
			_tag: "MoveCommitFile";
			sources: Array<Operand>;
			sourceCommitId: string;
			destinationCommitId: string;
	  }
	| {
			_tag: "SquashCommit";
			subjectCommitIds: Array<string>;
			destinationCommitId: string;
	  }
	| { _tag: "UndoCommit"; subjectCommitIds: Array<string>; assignTo: string | null }
	| {
			_tag: "DiscardChanges";
			sources: Array<Operand>;
			commitId: string;
			assignTo: string | null;
	  }
	| { _tag: "MoveBranch"; subjectBranch: string; targetBranch: string };

type LabelledOperation = { operation: Operation; label: string };

const executeOperation = async ({
	projectId,
	operation,
	resolveChanges,
	dryRun,
}: {
	projectId: string;
	operation: Operation;
	resolveChanges: (sources: Array<Operand>) => Promise<Array<DiffSpec> | null>;
	dryRun: boolean;
}) =>
	Match.value(operation).pipe(
		Match.tagsExhaustive({
			AmendCommit: async (operation) => {
				const changes = await resolveChanges(operation.sources);
				if (!changes) return null;
				return window.lite.commitAmend({
					projectId,
					commitId: operation.commitId,
					changes,
					changesSource: { type: "head" },
					dryRun,
				});
			},
			MoveCommitFile: async (operation) => {
				const changes = await resolveChanges(operation.sources);
				if (!changes) return null;
				return window.lite.commitMoveChangesBetween({
					projectId,
					sourceCommitId: operation.sourceCommitId,
					destinationCommitId: operation.destinationCommitId,
					changes,
					dryRun,
				});
			},
			SquashCommit: (operation) =>
				window.lite.commitSquash({
					projectId,
					subjectCommitIds: operation.subjectCommitIds,
					targetCommitId: operation.destinationCommitId,
					howToCombineMessages: "KeepBoth",
					dryRun,
				}),
			UndoCommit: (operation) =>
				window.lite.commitUncommit({
					projectId,
					subjectCommitIds: operation.subjectCommitIds,
					assignTo: operation.assignTo,
					dryRun,
				}),
			DiscardChanges: async (operation) => {
				const changes = await resolveChanges(operation.sources);
				if (!changes) return null;
				return window.lite.commitUncommitChanges({
					projectId,
					commitId: operation.commitId,
					assignTo: operation.assignTo,
					changes,
					dryRun,
				});
			},
			CreateCommit: async (operation) => {
				const changes = await resolveChanges(operation.sources);
				if (!changes) return null;
				return window.lite.commitCreate({
					projectId,
					relativeTo: operation.relativeTo,
					side: operation.side,
					changes,
					changesSource: { type: "head" },
					message: operation.message,
					dryRun,
				});
			},
			SplitCommit: async (operation) => {
				const changes = await resolveChanges(operation.sources);
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
			MoveCommit: (operation) =>
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
		sources: operation && "sources" in operation ? operation.sources : undefined,
	});

	return useQuery({
		enabled: !!operation,
		queryKey: ["dryRun", projectId, operation, changes],
		queryFn: () => {
			if (!operation) return null;
			return executeOperation({
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

export const useExecuteOperation = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (operation: Operation) =>
			executeOperation({
				projectId,
				operation,
				resolveChanges: (sources) => resolveDiffSpecs({ projectId, queryClient, sources }),
				dryRun: false,
			}),
		onSuccess: async (response, _input, _ctx, { client }) => {
			if (response) {
				syncCoreCaches(client, dispatch, projectId, response);

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

const isUncommittedChangesSource = (source: Operand): boolean =>
	operandFileParent(source)?._tag === "UncommittedChanges";

const commitIdFromFileSources = (sources: Array<Operand>): string | null => {
	const [source, ...rest] = sources;
	if (!source) return null;

	const parent = operandFileParent(source);
	if (parent?._tag !== "Commit") return null;

	const hasDisparateParent = rest.some((source) => {
		const otherParent = operandFileParent(source);
		return otherParent === null || !operandEquals(parent, otherParent);
	});
	return hasDisparateParent ? null : parent.commitId;
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
}: {
	sources: Array<Operand>;
	target: Operand;
}): LabelledOperation | null => {
	if (
		target._tag === "Commit" &&
		sources.length > 0 &&
		sources.every((source) => source._tag === "Commit")
	) {
		return {
			operation: {
				_tag: "SquashCommit",
				subjectCommitIds: sources.map((source) => source.commitId),
				destinationCommitId: target.commitId,
			},
			label: "Squash",
		};
	}

	if (
		target._tag === "UncommittedChanges" &&
		sources.length > 0 &&
		sources.every((source) => source._tag === "Commit")
	) {
		return {
			operation: {
				_tag: "UndoCommit",
				subjectCommitIds: sources.map((source) => source.commitId),
				assignTo: null,
			},
			label: "Uncommit",
		};
	}

	if (target._tag === "Commit" && sources.length > 0 && sources.every(isUncommittedChangesSource)) {
		return {
			operation: {
				_tag: "AmendCommit",
				commitId: target.commitId,
				sources,
			},
			label: "Amend",
		};
	}

	const sourceCommitId = commitIdFromFileSources(sources);
	if (sourceCommitId === null) return null;

	if (target._tag === "UncommittedChanges") {
		return {
			operation: {
				_tag: "DiscardChanges",
				commitId: sourceCommitId,
				assignTo: null,
				sources,
			},
			label: "Uncommit",
		};
	}

	if (target._tag === "Commit") {
		return {
			operation: {
				_tag: "MoveCommitFile",
				sourceCommitId,
				destinationCommitId: target.commitId,
				sources,
			},
			label: "Amend",
		};
	}

	return null;
};

const intoOperation = ({
	sources,
	target,
}: {
	sources: Array<Operand>;
	target: Operand;
}): LabelledOperation | null => {
	const squash = squashOperation({ sources, target });
	if (squash) return squash;

	if (
		target._tag === "Branch" &&
		sources.length > 0 &&
		sources.every((source) => source._tag === "Commit")
	) {
		return {
			operation: {
				_tag: "MoveCommit",
				subjectCommitIds: sources.map((source) => source.commitId),
				relativeTo: { type: "referenceBytes", subject: target.branchRef },
				side: "below",
			},
			label: "Move here",
		};
	}

	if (target._tag === "Branch" && sources.length > 0 && sources.every(isUncommittedChangesSource)) {
		return {
			operation: {
				_tag: "CreateCommit",
				relativeTo: { type: "referenceBytes", subject: target.branchRef },
				side: "below",
				sources,
				message: "",
			},
			label: "Commit here",
		};
	}

	return null;
};

// https://linear.app/gitbutler/issue/GB-1735/support-all-permutations-of-moving-branches-and-commits
const moveOperation = ({
	sources,
	target,
	side,
}: {
	sources: Array<Operand>;
	target: Operand;
	side: InsertSide;
}): LabelledOperation | null => {
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

	if (relativeTo && sources.length > 0 && sources.every((source) => source._tag === "Commit")) {
		return {
			operation: {
				_tag: "MoveCommit",
				subjectCommitIds: sources.map((source) => source.commitId),
				relativeTo,
				side,
			},
			label: Match.value(side).pipe(
				Match.when("above", () => "Move above"),
				Match.when("below", () => "Move below"),
				Match.exhaustive,
			),
		};
	}

	if (relativeTo && sources.length > 0 && sources.every(isUncommittedChangesSource)) {
		return {
			operation: {
				_tag: "CreateCommit",
				relativeTo,
				side,
				sources,
				message: "",
			},
			label: Match.value(side).pipe(
				Match.when("above", () => "Commit above"),
				Match.when("below", () => "Commit below"),
				Match.exhaustive,
			),
		};
	}

	const sourceCommitId = commitIdFromFileSources(sources);
	if (relativeTo && sourceCommitId !== null) {
		return {
			operation: {
				_tag: "SplitCommit",
				sourceCommitId,
				relativeTo,
				side,
				sources,
			},
			label: Match.value(side).pipe(
				Match.when("above", () => "Commit above"),
				Match.when("below", () => "Commit below"),
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
			({ source, target }): LabelledOperation => ({
				operation: {
					_tag: "MoveBranch",
					subjectBranch: decodeBytes(source.branchRef),
					targetBranch: decodeBytes(target.branchRef),
				},
				label: "Move above",
			}),
		),
		Match.orElse(() => null),
	);

	return branchMoveOperation;
};

export type Placement = "into" | "above" | "below";

const isOperationSourceEnabled = (source: Operand): boolean =>
	Match.value(source).pipe(
		Match.when({ _tag: "Hunk", isResultOfBinaryToTextConversion: true }, () => false),
		Match.orElse(() => true),
	);

export type OperationsByPlacement = Record<Placement, LabelledOperation | null>;

export const getOperations = (sources: Array<Operand>, target: Operand): OperationsByPlacement => {
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
		into: intoOperation({ sources, target }),
		above: moveOperation({ sources, target, side: "above" }),
		below: moveOperation({ sources, target, side: "below" }),
	};
};

export const getOperation = (x: {
	sources: Array<Operand>;
	target: Operand;
	placement: Placement;
}): LabelledOperation | null => getOperations(x.sources, x.target)[x.placement];
