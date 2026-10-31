import { classes } from "./classes.ts";
import { diffFileSpacing } from "./diffFileLayout.ts";
import type { ComponentProps, CSSProperties, FC } from "react";
import styles from "./DiffFileList.module.css";

/**
 * A diff's files as a column of `DiffFile` cards, spaced by
 * `diffFileSpacing` (`diffFileLayout.ts`). It sets `--diff-file-inset` on
 * itself for anything inside that lines up with the cards.
 *
 * The space above the first card stays while the diff scrolls: the cards'
 * headers stick that far below the scroller's edge, where the first card
 * began, and a strip of the pane's background covers the code passing above
 * them.
 *
 * Put it inside the scroller, such as a `ScrollArea`, rather than making it
 * the scroller: browsers inset sticky elements by a scroller's own padding,
 * which would add to that space.
 * @import import { DiffFileList } from "@gitbutler/ui-react/DiffFileList.tsx";
 */
export const DiffFileList: FC<ComponentProps<"div">> = (props) => (
	<div
		{...props}
		className={classes(props.className, styles.list)}
		// Cast, as in DiffFile: a host compiling this source doesn't see the package's own
		// CSSProperties augmentation.
		style={
			{
				"--diff-file-inset": `${diffFileSpacing.inset}px`,
				"--diff-file-top": `${diffFileSpacing.top}px`,
				"--diff-file-gap": `${diffFileSpacing.gap}px`,
				"--diff-file-sticky-top": `${diffFileSpacing.top}px`,
				gap: diffFileSpacing.gap,
				padding: `0 ${diffFileSpacing.inset}px ${diffFileSpacing.bottom}px`,
				...props.style,
			} as CSSProperties
		}
	/>
);
