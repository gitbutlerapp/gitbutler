import { type RubOperation } from "#ui/api/rub.ts";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { type HunkHeader, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";

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

/**
 * | SOURCE ↓ / TARGET →    | Changes  | Commit |
 * | ---------------------- | -------- | ------ |
 * | File/hunk from changes | Assign   | Amend  |
 * | File/hunk from commit  | Uncommit | Amend  |
 * | Commit                 | Uncommit | Squash |
 *
 * Note this is currently different from the CLI's definition of "rubbing",
 * which also includes move operations.
 * https://linear.app/gitbutler/issue/GB-1160/what-should-rubbing-a-branch-into-another-branch-do#comment-db2abdb7
 */
export const getRubOperation = ({
	sourceItem,
	target,
}: {
	sourceItem: SourceItem;
	target: ChangeUnit;
}): RubOperation | null =>
	Match.value(sourceItem).pipe(
		Match.tagsExhaustive({
			Branch: (): RubOperation | null => null,
			Commit: ({ commitId: sourceCommitId }) =>
				Match.value(target).pipe(
					Match.tagsExhaustive({
						Changes: ({ stackId }): RubOperation => ({
							_tag: "CommitUncommit",
							commitId: sourceCommitId,
							assignTo: stackId,
						}),
						Commit: ({ commitId: destinationCommitId }): RubOperation | null => {
							if (sourceCommitId === destinationCommitId) return null;
							return {
								_tag: "CommitSquash",
								sourceCommitId,
								destinationCommitId,
							};
						},
					}),
				),
			TreeChanges: ({ parent, changes: sourceChanges }) => {
				const changes = sourceChanges.map(({ change, hunkHeaders }) =>
					createDiffSpec(change, hunkHeaders),
				);

				return Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: ({ stackId: sourceStackId }) =>
							Match.value(target).pipe(
								Match.tagsExhaustive({
									Changes: ({ stackId: targetStackId }): RubOperation | null => {
										if (sourceStackId === targetStackId) return null;
										return {
											_tag: "AssignHunk",
											assignments: sourceChanges.flatMap(({ change, hunkHeaders }) =>
												hunkHeaders.map((hunkHeader) => ({
													pathBytes: change.pathBytes,
													hunkHeader,
													stackId: targetStackId,
												})),
											),
										};
									},
									Commit: ({ commitId }): RubOperation => ({
										_tag: "CommitAmend",
										commitId,
										changes,
									}),
								}),
							),
						Commit: ({ commitId: sourceCommitId }) =>
							Match.value(target).pipe(
								Match.tagsExhaustive({
									Changes: ({ stackId }): RubOperation => ({
										_tag: "CommitUncommitChanges",
										commitId: sourceCommitId,
										assignTo: stackId,
										changes,
									}),
									Commit: ({ commitId: destinationCommitId }): RubOperation | null => {
										if (sourceCommitId === destinationCommitId) return null;
										return {
											_tag: "CommitMoveChangesBetween",
											sourceCommitId,
											destinationCommitId,
											changes,
										};
									},
								}),
							),
					}),
				);
			},
		}),
	);
