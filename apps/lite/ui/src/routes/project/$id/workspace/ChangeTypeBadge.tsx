import { FileStatusBadge, type FileStatusType } from "#ui/components/FileStatusBadge.tsx";
import type { CodeViewDiffItem } from "@pierre/diffs";
import { Match } from "effect";
import type { FC } from "react";

/** Adapts the diff viewer's file type to the status the rest of the app speaks. */
export const ChangeTypeBadge: FC<{ type: CodeViewDiffItem<unknown>["fileDiff"]["type"] }> = ({
	type,
}) => (
	<FileStatusBadge
		fontSize={12}
		status={Match.value(type).pipe(
			Match.withReturnType<FileStatusType>(),
			Match.when("new", () => "Addition"),
			Match.whenOr("change", "rename-changed", () => "Modification"),
			Match.when("rename-pure", () => "Rename"),
			Match.when("deleted", () => "Deletion"),
			Match.exhaustive,
		)}
	/>
);
