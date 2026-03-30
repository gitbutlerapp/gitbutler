import { formatShortcutKeys, ShortcutActionBase, ShortcutBinding } from "#ui/shortcuts.ts";
import { createContext, type FC, use } from "react";
import { createPortal } from "react-dom";
import styles from "./-ShortcutsBar.module.css";

export const ShortcutsBarPortalContext = createContext<HTMLElement | null>(null);

const ShortcutsBar: FC<{
	label: string | null;
	items: Array<ShortcutBinding<ShortcutActionBase>>;
}> = ({ label, items }) => {
	if (items.length === 0) return null;

	return (
		<div className={styles.container}>
			{label != null && <span className={styles.scope}>{label}</span>}
			{items.map((item) => (
				<div key={item.id} className={styles.item}>
					<span className={styles.keys}>{formatShortcutKeys(item.keys)}</span>
					<span>{item.description}</span>
				</div>
			))}
		</div>
	);
};

export const PositionedShortcutsBar: FC<{
	label?: string | null;
	items: Array<ShortcutBinding<ShortcutActionBase>>;
}> = ({ items, label = null }) => {
	const element = use(ShortcutsBarPortalContext);
	if (!element) return null;

	return createPortal(<ShortcutsBar label={label} items={items} />, element);
};
