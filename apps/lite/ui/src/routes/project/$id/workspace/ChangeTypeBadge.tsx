import { Badge, type BadgeVariant } from "#ui/components/Badge.tsx";
import type { CodeViewDiffItem } from "@pierre/diffs";
import { Match } from "effect";
import type { FC } from "react";

export const ChangeTypeBadge: FC<{ type: CodeViewDiffItem<unknown>["fileDiff"]["type"] }> = ({
	type,
}) => {
	const [label, variant] = Match.value(type).pipe(
		Match.withReturnType<[string, BadgeVariant]>(),
		Match.when("new", () => ["Added", "safe"]),
		Match.whenOr("change", "rename-changed", () => ["Modified", "blue"]),
		Match.when("rename-pure", () => ["Renamed", "purple"]),
		Match.when("deleted", () => ["Deleted", "danger"]),
		Match.exhaustive,
	);

	return <Badge variant={variant}>{label}</Badge>;
};
