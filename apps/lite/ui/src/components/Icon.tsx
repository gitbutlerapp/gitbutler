import styles from "./Icon.module.css";
import { ComponentProps, FC } from "react";
import type { IconName } from "./iconNames";
import { classes } from "#ui/components/classes.ts";
import { assert } from "#ui/assert.ts";
import { icons } from "./icons.ts";

type Props = {
	name: IconName;
	size?: number;
} & ComponentProps<"i">;

export const Icon: FC<Props> = ({ name, size, ...props }) => (
	<i
		{...props}
		className={classes(props.className, styles.icon, name === "spinner" && styles.spinning)}
		data-icon
		aria-hidden
		style={size !== undefined ? { "--icon-size": `${size}px` } : undefined}
		// oxlint-disable-next-line react/no-danger
		dangerouslySetInnerHTML={{ __html: assert(icons.get(name)) }}
	/>
);
