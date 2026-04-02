import { type RubSource } from "#ui/api/rub.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { type RubOperation } from "#ui/Operation.ts";
import { type HunkHeader, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { rubOperationLabel } from "./-RubOperationLabel.ts";

export type TreeChangeWithHunkHeaders = {
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

export type SourceItem =
	| { _tag: "Commit"; commitId: string }
	| { _tag: "Branch"; anchorRef: Array<number> }
	| {
			_tag: "TreeChanges";
			parent: ChangeUnit;
			changes: Array<TreeChangeWithHunkHeaders>;
	  };

const rubSourceFor = (item: SourceItem): RubSource | null =>
	Match.value(item).pipe(
		Match.tag("Branch", (): RubSource | null => null),
		Match.tag("Commit", ({ commitId }): RubSource | null => ({
			_tag: "Commit",
			commitId,
		})),
		Match.tag("TreeChanges", ({ parent, changes }): RubSource | null => ({
			_tag: "TreeChanges",
			parent,
			changes,
		})),
		Match.exhaustive,
	);

export const getRubOperation = ({
	sourceItem,
	target,
}: {
	sourceItem: SourceItem;
	target: ChangeUnit;
}): RubOperation | null => {
	const rubSource = rubSourceFor(sourceItem);
	if (!rubSource) return null;
	const rubOperation: RubOperation = {
		source: rubSource,
		target,
	};
	if (rubOperationLabel(rubOperation) === null) return null;
	return rubOperation;
};
