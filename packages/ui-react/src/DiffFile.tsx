import { classes } from "./classes.ts";
import { diffFileHeaderHeight } from "./diffFileLayout.ts";
import type { ComponentProps, CSSProperties, FC, ReactNode } from "react";
import styles from "./DiffFile.module.css";

type Props = {
	/** The file's header, usually a `DiffFileHeader`. It sticks to the top of the scroller while its file is in view. */
	header: ReactNode;
	/** Drops the diff and leaves the header. */
	collapsed?: boolean;
	/** How far below the scroller's top edge the header sticks, for a scroller with a bar pinned over it. Inside a `DiffFileList` it defaults to the list's top spacing, elsewhere to 0. */
	stickyTop?: number;
	/** The diff itself: whatever the host renders it with, its own file header turned off. */
	children?: ReactNode;
} & Omit<ComponentProps<"section">, "children">;

/**
 * One file of a diff as a card: its header, and the diff under it. While the
 * reader scrolls through the file the header stays at the top of the
 * scroller. As the file scrolls away the card shrinks from the bottom around
 * the still header, its corners easing flat, so the frame stays whole until
 * the next file takes the top.
 *
 * The card doesn't draw the diff. The host renders it as the children with
 * its renderer's own header off (Pierre's `disableFileHeader`), so the card
 * works with any renderer and the library stays free of one. Lay cards out
 * with `DiffFileList`.
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
	<section
		{...props}
		className={classes(props.className, styles.file)}
		// Cast, as the library's other components do: a host compiling this source doesn't
		// see the package's own CSSProperties augmentation.
		style={
			{
				"--diff-file-header-height": `${diffFileHeaderHeight}px`,
				...(stickyTop !== undefined && { "--diff-file-sticky-top": `${stickyTop}px` }),
				...props.style,
			} as CSSProperties
		}
	>
		<div className={styles.header}>{header}</div>
		{!collapsed && <div className={styles.body}>{children}</div>}
	</section>
);
