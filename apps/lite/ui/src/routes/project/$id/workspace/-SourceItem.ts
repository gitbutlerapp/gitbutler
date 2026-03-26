import { type RubSource } from "#ui/api/rub.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { type RubOperation } from "#ui/Operation.ts";
import { type HunkHeader, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { rubOperationLabel } from "./-RubOperationLabel.ts";

export type SourceItem =
	| { _tag: "Commit"; commitId: string }
	| { _tag: "Branch"; anchorRef: Array<number> }
	| {
			_tag: "TreeChange";
			source: {
				parent: ChangeUnit;
				change: TreeChange;
				hunkHeaders: Array<HunkHeader>;
			};
	  };

const rubSourceFor = (item: SourceItem): RubSource | null =>
	Match.value(item).pipe(
		Match.tag("Branch", (): RubSource | null => null),
		Match.tag("Commit", ({ commitId }): RubSource | null => ({
			_tag: "Commit",
			source: { commitId },
		})),
		Match.tag("TreeChange", ({ source }): RubSource | null => ({
			_tag: "TreeChange",
			source,
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
