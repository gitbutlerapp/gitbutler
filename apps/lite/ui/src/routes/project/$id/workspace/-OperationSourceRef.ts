import { type FileParent } from "#ui/domain/FileParent.ts";
import { type HunkHeader } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { Item } from "./-Item.ts";

export type OperationSourceRef =
	| { _tag: "Branch"; ref: Array<number> }
	| { _tag: "Commit"; commitId: string }
	| { _tag: "ChangesSection"; stackId: string | null }
	| { _tag: "File"; parent: FileParent; path: string }
	| { _tag: "Hunk"; parent: FileParent; path: string; hunkHeader: HunkHeader };

export const operationSourceRefFromItem = (item: Item): OperationSourceRef | null =>
	Match.value(item).pipe(
		Match.tags({
			ChangesSection: ({ stackId }): OperationSourceRef => ({ _tag: "ChangesSection", stackId }),
			Change: ({ stackId, path }): OperationSourceRef => ({
				_tag: "File",
				parent: { _tag: "ChangesSection", stackId },
				path,
			}),
			Commit: ({ commitId }): OperationSourceRef => ({ _tag: "Commit", commitId }),
			BaseCommit: ({ commitId }): OperationSourceRef => ({ _tag: "Commit", commitId }),
		}),
		Match.orElse(() => null),
	);
