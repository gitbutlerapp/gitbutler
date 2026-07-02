import type { TreeChange } from "@gitbutler/but-sdk";
import { Array } from "effect";

type ChangeFileRowItem = {
	change: TreeChange;
	dependencyCommitIds?: Array.NonEmptyArray<string>;
	path: string;
};

export const changeFileRowItem = ({
	change,
	dependencyCommitIds,
	path,
}: ChangeFileRowItem): FileRowItem => ({
	_tag: "Change",
	change,
	dependencyCommitIds,
	path,
});

type ConflictFileRowItem = {
	path: string;
};

export const conflictFileRowItem = ({ path }: ConflictFileRowItem): FileRowItem => ({
	_tag: "Conflict",
	path,
});

export type FileRowItem =
	| ({ _tag: "Change" } & ChangeFileRowItem)
	| ({ _tag: "Conflict" } & ConflictFileRowItem);
