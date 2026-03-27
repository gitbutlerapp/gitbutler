import {
	formatShortcutKeys,
	globalShortcutBindings,
	ShortcutActionBase,
	type ShortcutBinding,
} from "#ui/shortcuts.ts";
import { createContext, type FC, use } from "react";
import { createPortal } from "react-dom";
import styles from "./-ShortcutsBar.module.css";

export const ShortcutsBarPortalContext = createContext<HTMLElement | null>(null);

type ShortcutsBarItem = Pick<
	ShortcutBinding<ShortcutActionBase, unknown>,
	"id" | "description" | "keys"
>;

export type ShortcutsBarMode = { label: string | null; items: Array<ShortcutsBarItem> };

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
