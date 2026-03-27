import { formatShortcutKeys, globalShortcutBindings, type ShortcutBinding } from "#ui/shortcuts.ts";
import { Match } from "effect";
import { createContext, type FC, use } from "react";
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
import styles from "./-ShortcutsBar.module.css";

export const ShortcutsBarPortalContext = createContext<HTMLElement | null>(null);

type ShortcutsBarItem = Pick<ShortcutBinding<unknown, unknown>, "id" | "description" | "keys">;

type ShortcutsBarMode = { label: string | null; items: Array<ShortcutsBarItem> };

export const getShortcutsBarMode = ({
	selection,
	commitDetailsSelection,
	editingCommit,
}: {
	selection: Item | null;
	commitDetailsSelection: CommitDetailsSelection | null;
	editingCommit: EditingCommit | null;
}): ShortcutsBarMode | null => {
	if (selection === null) return null;

	return Match.value(selection).pipe(
		Match.tag(
			"Changes",
			(selection): ShortcutsBarMode => ({
				label: "changes",
				items: changesSelectionBindings.filter((binding) => binding.when?.(selection) ?? true),
			}),
		),
		Match.tag("Commit", (selection): ShortcutsBarMode => {
			if (
				editingCommit !== null &&
				editingCommit.stackId === selection.stackId &&
				editingCommit.segmentIndex === selection.segmentIndex &&
				editingCommit.commitId === selection.commitId
			)
				return {
					label: "edit message",
					items: commitEditingMessageBindings,
				};

			if (commitDetailsSelection !== null)
				return {
					label: "commit details",
					items: commitDetailsSelectionBindings.filter(
						(binding) => binding.when?.(undefined) ?? true,
					),
				};

			return {
				label: "commit",
				items: commitSelectionBindings.filter((binding) => binding.when?.(selection) ?? true),
			};
		}),
		Match.tag(
			"BaseCommit",
			(): ShortcutsBarMode => ({
				label: "base commit",
				items: segmentSelectionBindings.filter((binding) => binding.when?.(undefined) ?? true),
			}),
		),
		Match.tag(
			"Segment",
			(): ShortcutsBarMode => ({
				label: "segment",
				items: segmentSelectionBindings.filter((binding) => binding.when?.(undefined) ?? true),
			}),
		),
		Match.exhaustive,
	);
};

const ShortcutsBar: FC<{
	mode?: ShortcutsBarMode | null;
}> = ({ mode = null }) => {
	const items: Array<ShortcutsBarItem> =
		mode === null ? globalShortcutBindings : [...mode.items, ...globalShortcutBindings];

	if (items.length === 0) return null;

	return (
		<div className={styles.shortcutsBar}>
			{mode?.label != null && <span className={styles.shortcutsBarMode}>{mode.label}</span>}
			{items.map((item) => (
				<div key={item.id} className={styles.shortcutsBarItem}>
					<span className={styles.shortcutsBarKeys}>{formatShortcutKeys(item.keys)}</span>
					<span>{item.description}</span>
				</div>
			))}
		</div>
	);
};

export const PositionedShortcutsBar: FC<{
	mode?: ShortcutsBarMode | null;
}> = ({ mode = null }) => {
	const shortcutsBarPortalNode = use(ShortcutsBarPortalContext);
	if (!shortcutsBarPortalNode) return null;

	return createPortal(<ShortcutsBar mode={mode} />, shortcutsBarPortalNode);
};
