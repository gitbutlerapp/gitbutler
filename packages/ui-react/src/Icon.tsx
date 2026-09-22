import styles from "./Icon.module.css";
import type { ComponentProps, FC } from "react";
import type { IconName } from "./iconNames";
import { classes } from "./classes.ts";
import { assert } from "./assert.ts";
import { icons } from "./icons.ts";

type Props = {
	name: IconName;
	size?: number;
} & ComponentProps<"i">;

/**
 * @import import { Icon } from "@gitbutler/ui-react/Icon.tsx";
 */
export const Icon: FC<Props> = ({ name, size, ...props }) => (
	<i
		{...props}
		className={classes(props.className, styles.icon, name === "spinner" && styles.spinning)}
		data-icon
		aria-hidden
		style={{
			...props.style,
			...(size !== undefined ? { "--icon-size": `${size}px` } : undefined),
		}}
		// oxlint-disable-next-line react/no-danger -- SVGs are bundled app assets.
		dangerouslySetInnerHTML={{ __html: assert(icons.get(name)) }}
	/>
);
