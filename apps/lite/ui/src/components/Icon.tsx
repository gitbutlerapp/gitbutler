import styles from "./Icon.module.css";
import { FC } from "react";
import type { IconName } from "./iconNames";
import { classes } from "#ui/components/classes.ts";
import { assert } from "#ui/assert.ts";

const svgModules = import.meta.glob("./icons/*.svg", {
	query: "?raw",
	import: "default",
	eager: true,
}) as Record<string, string>;

/** @internal */
export const icons: Map<IconName, string> = new Map();
for (const [path, svg] of Object.entries(svgModules)) {
	const name = path.replace(/^.*\/(.+)\.svg$/, "$1") as IconName;
	icons.set(name, svg);
}

type Props = {
	name: IconName;
	size?: number;
};

export const Icon: FC<Props> = ({ name, size }) => (
	<i
		className={classes(styles.icon, name === "spinner" && styles.spinning)}
		data-icon
		aria-hidden
		style={size !== undefined ? { "--icon-size": `${size}px` } : undefined}
		// oxlint-disable-next-line react/no-danger
		dangerouslySetInnerHTML={{ __html: assert(icons.get(name)) }}
	/>
);
