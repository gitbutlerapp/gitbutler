import { formatShortcutKeys, globalShortcutBindings, type ShortcutBinding } from "#ui/shortcuts.ts";
import { ShortcutBarPortalContext } from "#ui/routes/-ShortcutBarContext.tsx";
import { Match } from "effect";
import { type FC, use } from "react";
import { createPortal } from "react-dom";
import {
	commitDetailsSelectionBindings,
	type CommitDetailsSelection,
} from "./-CommitDetailsSelection.ts";
import { type EditingCommit } from "./-EditingCommit.ts";
import { type Item } from "./-Item.ts";
import {
	changesSelectionBindings,
	commitEditingMessageBindings,
	commitSelectionBindings,
	segmentSelectionBindings,
} from "./-Selection.ts";
import styles from "./-ShortcutBar.module.css";

type ShortcutBarItem = Pick<ShortcutBinding<unknown, unknown>, "id" | "description" | "keys">;

type ShortcutBarMode = { label: string | null; items: Array<ShortcutBarItem> };

export const getShortcutBarMode = ({
	selection,
	commitDetailsSelection,
	editingCommit,
}: {
	selection: Item | null;
	commitDetailsSelection: CommitDetailsSelection | null;
	editingCommit: EditingCommit | null;
}): ShortcutBarMode | null => {
	if (selection === null) return null;

	if (
		selection._tag === "Commit" &&
		editingCommit !== null &&
		editingCommit.stackId === selection.stackId &&
		editingCommit.segmentIndex === selection.segmentIndex &&
		editingCommit.commitId === selection.commitId
	)
		return {
			label: "edit message",
			items: commitEditingMessageBindings,
		};

	if (selection._tag === "Commit" && commitDetailsSelection !== null)
		return {
			label: "commit details",
			items: commitDetailsSelectionBindings.filter((binding) => binding.when?.(undefined) ?? true),
		};

	return Match.value(selection).pipe(
		Match.tag(
			"Changes",
			(selection): ShortcutBarMode => ({
				label: "changes",
				items: changesSelectionBindings.filter((binding) => binding.when?.(selection) ?? true),
			}),
		),
		Match.tag(
			"Commit",
			(selection): ShortcutBarMode => ({
				label: "commit",
				items: commitSelectionBindings.filter((binding) => binding.when?.(selection) ?? true),
			}),
		),
		Match.tag(
			"Segment",
			(): ShortcutBarMode => ({
				label: "segment",
				items: segmentSelectionBindings.filter((binding) => binding.when?.(undefined) ?? true),
			}),
		),
		Match.exhaustive,
	);
};

const ShortcutBar: FC<{
	mode?: ShortcutBarMode | null;
}> = ({ mode = null }) => {
	const items: Array<ShortcutBarItem> =
		mode === null ? globalShortcutBindings : [...mode.items, ...globalShortcutBindings];

	if (items.length === 0) return null;

	return (
		<div className={styles.shortcutBar}>
			{mode?.label != null && <span className={styles.shortcutBarMode}>{mode.label}</span>}
			{items.map((item) => (
				<div key={item.id} className={styles.shortcutBarItem}>
					<span className={styles.shortcutBarKeys}>{formatShortcutKeys(item.keys)}</span>
					<span>{item.description}</span>
				</div>
			))}
		</div>
	);
};

export const PositionedShortcutBar: FC<{
	mode?: ShortcutBarMode | null;
}> = ({ mode = null }) => {
	const shortcutBarPortalNode = use(ShortcutBarPortalContext);
	if (!shortcutBarPortalNode) return null;

	return createPortal(<ShortcutBar mode={mode} />, shortcutBarPortalNode);
};
