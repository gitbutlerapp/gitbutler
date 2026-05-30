import styles from "./Icon.module.css";
import { FC } from "react";
import type { IconName } from "./iconNames";
import { classes } from "#ui/components/classes.ts";

const svgModules = import.meta.glob("./icons/*.svg", {
	query: "?raw",
	import: "default",
	eager: true,
}) as Record<string, string>;

const icons: Record<string, string> = {};
for (const [path, svg] of Object.entries(svgModules)) {
	const name = path.replace(/^.*\/(.+)\.svg$/, "$1");
	icons[name] = svg;
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
		dangerouslySetInnerHTML={{ __html: icons[name] ?? "" }}
	/>
);
