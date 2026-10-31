import { classes } from "./classes.ts";
import type { CSSProperties, FC } from "react";
import styles from "./SkeletonBone.module.css";

type Props =
	| {
			/** A line of text that is still loading. */
			shape?: "text";
			/** How much of the line the text would fill. Defaults to all of it. */
			width?: CSSProperties["width"];
			className?: string;
	  }
	| {
			/** An avatar or an icon that is still loading. */
			shape: "circle";
			/** The diameter, in pixels: the size of what it stands in for. */
			size?: number;
			className?: string;
	  };

/**
 * A placeholder for content that is still loading, pulsing until it lands. Draw the row as it
 * will look, with a bone where each piece of text or picture goes, so nothing moves when the
 * content arrives.
 *
 * A `text` bone takes its size from the text style around it: give it the `text-NN` class the
 * text will have and it is one line tall, with a bar the height of the font in the middle. A
 * `circle` bone stands in for an avatar or an icon at that size.
 *
 * Bones are hidden from screen readers; mark the container that is loading `aria-busy`.
 * @import import { SkeletonBone } from "@gitbutler/ui-react/SkeletonBone.tsx";
 */
export const SkeletonBone: FC<Props> = (props) =>
	props.shape === "circle" ? (
		<span
			aria-hidden
			className={classes(styles.bone, styles.circle, props.className)}
			style={{ "--bone-size": `${props.size ?? 16}px` } as CSSProperties}
		/>
	) : (
		<span
			aria-hidden
			className={classes(styles.line, props.className)}
			style={{ width: props.width }}
		>
			<span className={classes(styles.bone, styles.bar)} />
		</span>
	);
