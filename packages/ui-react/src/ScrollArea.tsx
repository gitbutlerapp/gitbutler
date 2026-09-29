import { ScrollArea as BaseScrollArea } from "@base-ui/react";
import type { ComponentProps, FC, Ref } from "react";
import { classes } from "./classes.ts";
import styles from "./ScrollArea.module.css";

/**
 * A pane's scroll container, with a thin scrollbar drawn over the content rather than a gutter
 * beside it. The thumb shows while the area is scrolled or under the pointer, and fades away at
 * rest.
 *
 * Two elements, so their classes go to the right one: `className` places the area in the layout
 * that holds it (its growth, its height, a border), and `viewportClassName` lays out what scrolls
 * inside it (a flex column, padding). The viewport is the element that scrolls: give a virtualizer,
 * or anything that measures or restores scrolling, `viewportRef`.
 *
 * `separator` draws a hairline across the top while the content is scrolled down, so the rows
 * passing under a sticky header read as passing under it.
 *
 * @public
 * @import import { ScrollArea } from "@gitbutler/ui-react/ScrollArea.tsx";
 */
export const ScrollArea: FC<
	{
		viewportClassName?: string;
		viewportRef?: Ref<HTMLDivElement>;
		separator?: boolean;
	} & ComponentProps<"div">
> = ({ viewportClassName, viewportRef, separator = false, className, children, ...props }) => (
	<BaseScrollArea.Root
		{...props}
		className={classes(className, styles.root, separator && styles.separator)}
	>
		<BaseScrollArea.Viewport
			ref={viewportRef}
			className={classes(viewportClassName, styles.viewport)}
		>
			{children}
		</BaseScrollArea.Viewport>
		<BaseScrollArea.Scrollbar orientation="vertical" className={styles.scrollbar}>
			<BaseScrollArea.Thumb className={styles.thumb} onPointerMove={ignoreButtonlessDrag} />
		</BaseScrollArea.Scrollbar>
		<BaseScrollArea.Scrollbar orientation="horizontal" className={styles.scrollbar}>
			<BaseScrollArea.Thumb className={styles.thumb} onPointerMove={ignoreButtonlessDrag} />
		</BaseScrollArea.Scrollbar>
	</BaseScrollArea.Root>
);

/**
 * Base UI drags from a thumb's pointer-down to its pointer-up, and a pointer-up it misses leaves
 * the drag running: every later move over the thumb then scrolls back towards where the drag
 * began. A move with no button held is no drag, so it never reaches that handler.
 */
const ignoreButtonlessDrag = (
	event: Parameters<NonNullable<BaseScrollArea.Thumb.Props["onPointerMove"]>>[0],
) => {
	if (event.buttons === 0) event.preventBaseUIHandler();
};
