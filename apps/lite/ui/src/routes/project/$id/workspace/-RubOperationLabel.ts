import { type RubOperation } from "#ui/Operation.ts";
import { Match } from "effect";

type RubOperationLabel = "Amend" | "Uncommit" | "Assign" | "Unassign" | "Squash";

/**
 * | SOURCE ↓ / TARGET →    | Unassigned changes | Assigned changes | Commit |
 * | ---------------------- | ------------------ | ---------------- | ------ |
 * | File/hunk from changes | Unassign           | Assign           | Amend  |
 * | File/hunk from commit  | Uncommit           | Uncommit         | Amend  |
 * | Commit                 | Uncommit           | Uncommit         | Squash |
 *
 * Note this is currently different from the CLI's definition of "rubbing" which
 * includes move operations.
 * https://linear.app/gitbutler/issue/GB-1160/what-should-rubbing-a-branch-into-another-branch-do#comment-db2abdb7
 */
export const rubOperationLabel = ({ source, target }: RubOperation): RubOperationLabel | null =>
	Match.value(source).pipe(
		Match.withReturnType<RubOperationLabel | null>(),
		Match.tag("TreeChanges", (source) =>
			Match.value(source.parent).pipe(
				Match.withReturnType<RubOperationLabel | null>(),
				Match.tag("Changes", (source) =>
					Match.value(target).pipe(
						Match.withReturnType<RubOperationLabel | null>(),
						Match.tag("Changes", (target) => {
							if (source.stackId === target.stackId) return null;
							return target.stackId === null ? "Unassign" : "Assign";
						}),
						Match.tag("Commit", () => "Amend"),
						Match.exhaustive,
					),
				),
				Match.tag("Commit", (source) =>
					Match.value(target).pipe(
						Match.withReturnType<RubOperationLabel | null>(),
						Match.tag("Changes", () => "Uncommit"),
						Match.tag("Commit", (target) => {
							if (source.commitId === target.commitId) return null;
							return "Amend";
						}),
						Match.exhaustive,
					),
				),
				Match.exhaustive,
			),
		),
		Match.tag("Commit", (source) =>
			Match.value(target).pipe(
				Match.withReturnType<RubOperationLabel | null>(),
				Match.tag("Changes", () => "Uncommit"),
				Match.tag("Commit", (target) => {
					if (source.commitId === target.commitId) return null;
					return "Squash";
				}),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
