import { Match } from "effect";
import { type OperationSource, operationSourceMatchesItem } from "./OperationSource.ts";
import { branchItem, itemEquals } from "./Item.ts";
import { type NavigationIndex } from "./WorkspaceModel.ts";

/** @public */
export type RubOperationMode = { source: OperationSource };
/** @public */
export type MoveOperationMode = { source: OperationSource };
export type OperationMode =
	| ({ _tag: "Rub" } & RubOperationMode)
	| ({ _tag: "Move" } & MoveOperationMode);

/** @public */
export type RewordCommitWorkspaceMode = { commitId: string };
/** @public */
export type RenameBranchWorkspaceMode = { stackId: string; branchRef: Array<number> };
export type WorkspaceMode =
	| { _tag: "Default" }
	| ({ _tag: "RewordCommit" } & RewordCommitWorkspaceMode)
	| ({ _tag: "RenameBranch" } & RenameBranchWorkspaceMode)
	| OperationMode;

/** @public */
export const rubOperationMode = ({ source }: RubOperationMode): OperationMode => ({
	_tag: "Rub",
	source,
});

/** @public */
export const moveOperationMode = ({ source }: MoveOperationMode): OperationMode => ({
	_tag: "Move",
	source,
});

/** @public */
export const defaultWorkspaceMode: WorkspaceMode = {
	_tag: "Default",
};

/** @public */
export const rewordCommitWorkspaceMode = ({
	commitId,
}: RewordCommitWorkspaceMode): WorkspaceMode => ({
	_tag: "RewordCommit",
	commitId,
});

/** @public */
export const renameBranchWorkspaceMode = ({
	stackId,
	branchRef,
}: RenameBranchWorkspaceMode): WorkspaceMode => ({
	_tag: "RenameBranch",
	stackId,
	branchRef,
});

export const getOperationMode = (mode: WorkspaceMode): OperationMode | null =>
	mode._tag === "Rub" || mode._tag === "Move" ? mode : null;

export const normalizeWorkspaceMode = ({
	mode,
	navigationIndex,
}: {
	mode: WorkspaceMode;
	navigationIndex: NavigationIndex;
}): WorkspaceMode =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => mode,
			Rub: (mode) =>
				navigationIndex.items.some((item) => operationSourceMatchesItem(mode.source, item))
					? mode
					: defaultWorkspaceMode,
			Move: (mode) =>
				navigationIndex.items.some((item) => operationSourceMatchesItem(mode.source, item))
					? mode
					: defaultWorkspaceMode,
			RewordCommit: (mode) =>
				navigationIndex.items.some(
					(item) => item._tag === "Commit" && item.commitId === mode.commitId,
				)
					? mode
					: defaultWorkspaceMode,
			RenameBranch: (mode) =>
				navigationIndex.items.some((item) =>
					itemEquals(
						item,
						branchItem({
							stackId: mode.stackId,
							branchRef: mode.branchRef,
						}),
					),
				)
					? mode
					: defaultWorkspaceMode,
		}),
	);
