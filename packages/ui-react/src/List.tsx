import { classes } from "./classes.ts";
import styles from "./List.module.css";
import type { ComponentProps, FC, ReactNode } from "react";

/**
 * A short, read-only list: the files a prompt acts on, the reasons a push was rejected, the checks
 * a merge waits on. Stacks its items with no indent beyond their markers. For rows that do
 * something, use a row or {@link PopupItem}; authored lists in a description keep Markdown's own
 * bullets.
 *
 * @public
 * @import import { List, ListItem } from "@gitbutler/ui-react/List.tsx";
 */
export const List: FC<ComponentProps<"ul">> = ({ className, ...props }) => (
	<ul {...props} className={classes("text-13", styles.list, className)} />
);

/**
 * One item of a {@link List}: a marker, then the text, which wraps under itself rather than under
 * the marker.
 *
 * The marker is a small dot unless the item says what kind of thing it is: a `FileIcon` for a
 * file, an `Icon` for a branch or a commit. The marker sits on the item's first line and is hidden
 * from screen readers, since the list already reads as a list.
 *
 * @public
 * @import import { List, ListItem } from "@gitbutler/ui-react/List.tsx";
 */
export const ListItem: FC<{ marker?: ReactNode } & ComponentProps<"li">> = ({
	marker,
	children,
	className,
	...props
}) => (
	<li {...props} className={classes(styles.item, className)}>
		<span className={styles.marker} aria-hidden="true">
			{marker ?? <span className={styles.dot} />}
		</span>
		<span className={styles.content}>{children}</span>
	</li>
);
