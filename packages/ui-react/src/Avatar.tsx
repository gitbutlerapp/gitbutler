import { classes } from "./classes.ts";
import type { CSSProperties, FC } from "react";
import styles from "./Avatar.module.css";

/** The sizes a person's picture is drawn at: 14 in a compact row, 16 inline, 18 heading a card. */
export type AvatarSize = 14 | 16 | 18;

/**
 * A person's picture in a circle, or an empty circle on `--bg-2` when they have none, so
 * a row keeps its rhythm either way. Decorative by default: the name beside it is what a
 * screen reader reads, so give `alt` only where the picture stands alone.
 * @import import { Avatar } from "@gitbutler/ui-react/Avatar.tsx";
 */
export const Avatar: FC<{
	src: string | null | undefined;
	size?: AvatarSize;
	alt?: string;
	className?: string;
}> = ({ src, size = 16, alt = "", className }) => {
	const style = { "--avatar-size": `${size}px` } as CSSProperties;
	return src != null && src !== "" ? (
		<img src={src} alt={alt} className={classes(styles.avatar, className)} style={style} />
	) : (
		<span className={classes(styles.avatar, className)} style={style} />
	);
};
