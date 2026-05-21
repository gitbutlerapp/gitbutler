import { FC } from "react";
import type { IconName } from "./iconNames";

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

export const Icon: FC<Props> = ({ name, size = 16 }) => (
	<i
		className="icon"
		aria-hidden="true"
		style={{ display: "inline-flex", width: size, height: size }}
		// oxlint-disable-next-line react/no-danger
		dangerouslySetInnerHTML={{ __html: icons[name] ?? "" }}
	/>
);
