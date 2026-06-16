import type { TreeChange } from "@gitbutler/but-sdk";
import { Array } from "effect";

type ChangeFileTreeItem = {
	change: TreeChange;
	dependencyCommitIds?: Array.NonEmptyArray<string>;
	path: string;
};

export const changeFileTreeItem = ({
	change,
	dependencyCommitIds,
	path,
}: ChangeFileTreeItem): FileTreeItem => ({
	_tag: "Change",
	change,
	dependencyCommitIds,
	path,
});

type ConflictFileTreeItem = {
	path: string;
};

export const conflictFileTreeItem = ({ path }: ConflictFileTreeItem): FileTreeItem => ({
	_tag: "Conflict",
	path,
});

export type FileTreeItem =
	| ({ _tag: "Change" } & ChangeFileTreeItem)
	| ({ _tag: "Conflict" } & ConflictFileTreeItem);
