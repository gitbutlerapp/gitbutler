import { classes } from "./classes.ts";
import type { ComponentProps, FC, ReactNode } from "react";
import styles from "./DiffFile.module.css";

type Props = {
	/** The file's header, usually a `DiffFileHeader`. It sticks to the top of the scroller while its file is in view. */
	header: ReactNode;
	/** Drops the diff and leaves the header. */
	collapsed?: boolean;
	/** How far below the scroller's top edge the header sticks, for a scroller with a bar pinned over it. */
	stickyTop?: number;
	/** The diff itself: whatever the host renders it with, its own file header turned off. */
	children?: ReactNode;
} & Omit<ComponentProps<"section">, "children">;

/**
 * One file of a diff as a card: its header, and the diff under it. While the
 * reader scrolls through the file the header stays at the top of the
 * scroller; the next file's header pushes it off as that file arrives.
 *
 * The card doesn't draw the diff. The host renders it as the children with
 * its renderer's own header off (Pierre's `disableFileHeader`), so the card
 * works with any renderer and the library stays free of one.
 *
 * The header sticks to the nearest scrolling ancestor, so nothing between the
 * card and that scroller may clip its overflow.
 * @import import { DiffFile } from "@gitbutler/ui-react/DiffFile.tsx";
 */
export const DiffFile: FC<Props> = ({
	header,
	collapsed = false,
	stickyTop,
	children,
	...props
}) => (
	<section {...props} className={classes(props.className, styles.file)}>
		<div className={styles.header} style={{ top: stickyTop }}>
			{header}
		</div>
		{!collapsed && children}
	</section>
);
