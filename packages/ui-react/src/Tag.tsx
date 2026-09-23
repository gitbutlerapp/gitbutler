import type { BadgeSize } from "./Badge.tsx";
import { classes } from "./classes.ts";
import type { CSSProperties, FC } from "react";
import styles from "./Tag.module.css";

/**
 * A named, coloured category: a label on a pull request, an issue's label, a topic.
 * An outlined pill with the name in the text colour and the colour as a dot, so any
 * colour reads on any ground and a row of them stays calm. Without a colour, no dot.
 * @import import { Tag } from "@gitbutler/ui-react/Tag.tsx";
 */
export const Tag: FC<{
	name: string;
	/** The category's colour, as hex with or without the `#` (forges send both). */
	color?: string | null;
	/** Shown on hover after the name, as a forge's label description is. */
	description?: string | null;
	size?: BadgeSize;
	className?: string;
}> = ({ name, color, description, size = "large", className }) => {
	const dot = color == null || color === "" ? null : color.startsWith("#") ? color : `#${color}`;
	return (
		<span
			className={classes(styles.tag, styles[size], className)}
			title={description != null && description !== "" ? `${name}: ${description}` : name}
		>
			{dot !== null && (
				<span className={styles.dot} style={{ "--tag-color": dot } as CSSProperties} />
			)}
			<span className={styles.name}>{name}</span>
		</span>
	);
};
