import { classes } from "#ui/components/classes.ts";
import { ScrollArea } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import styles from "./ui.module.css";

/**
 * A scroll container based on Base UI's ScrollArea with the standard overlay
 * scrollbar. Unlike a native scrollbar, it doesn't reserve gutter space, so
 * content and sticky separators can span the full viewport width.
 *
 * `className` applies to the root (use it for sizing within the parent
 * layout), `viewportClassName` to the scroll viewport (use it for the
 * content padding).
 */
export const Scroller: FC<
	{
		className?: string;
		viewportClassName?: string;
		/** Show a separator line at the top edge while scrolled down. */
		withSeparator?: boolean;
	} & Omit<ComponentProps<typeof ScrollArea.Root>, "className">
> = ({ className, viewportClassName, withSeparator = false, children, ...props }) => (
	<ScrollArea.Root {...props} className={classes(styles.scrollAreaRoot, className)}>
		<ScrollArea.Viewport
			className={classes(
				styles.scrollAreaViewport,
				withSeparator && styles.scrollerWithSeparator,
				viewportClassName,
			)}
		>
			{children}
		</ScrollArea.Viewport>
		<ScrollArea.Scrollbar className={styles.scrollAreaScrollbar}>
			<ScrollArea.Thumb className={styles.scrollAreaThumb} />
		</ScrollArea.Scrollbar>
	</ScrollArea.Root>
);
