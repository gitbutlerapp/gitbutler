import { globalShortcutBindings, type ShortcutBinding } from "#ui/shortcuts.ts";
import { ShortcutBarPortalContext } from "#ui/routes/-ShortcutBarContext.tsx";
import { Match } from "effect";
import { type FC, use } from "react";
import { createPortal } from "react-dom";
import {
	commitDetailsSelectionBindings,
	type CommitDetailsSelection,
} from "./-CommitDetailsSelection.ts";
import { type Item } from "./-Item.ts";
import {
	changesSelectionBindings,
	commitSelectionBindings,
	segmentSelectionBindings,
} from "./-Selection.ts";
import styles from "./-ShortcutBar.module.css";

const formatShortcutKeys = (keys: Array<string>): string =>
	keys
		.map((key) =>
			Match.value(key).pipe(
				Match.when("ArrowUp", () => "↑"),
				Match.when("ArrowDown", () => "↓"),
				Match.when("ArrowLeft", () => "←"),
				Match.when("ArrowRight", () => "→"),
				Match.when("Escape", () => "esc"),
				Match.orElse(() => key.toLowerCase()),
			),
		)
		.join("/");

type ShortcutBarItem = Pick<ShortcutBinding<unknown, unknown>, "id" | "description" | "keys">;

export type ShortcutBarMode = { label: string | null; items: Array<ShortcutBarItem> };

export const getShortcutBarMode = ({
	selection,
	commitDetailsSelection,
}: {
	selection: Item | null;
	commitDetailsSelection: CommitDetailsSelection | null;
}): ShortcutBarMode | null => {
	if (selection === null) return null;

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

export const ShortcutBar: FC<{
	mode?: ShortcutBarMode | null;
}> = ({ mode = null }) => {
	const items: Array<ShortcutBarItem> =
		mode === null ? globalShortcutBindings : [...mode.items, ...globalShortcutBindings];

	if (items.length === 0) return null;

	return (
		<div className={styles.shortcutBar}>
			{mode?.label && <span className={styles.shortcutBarMode}>{mode.label}</span>}
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
