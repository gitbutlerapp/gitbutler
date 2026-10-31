import type { IconName } from "./iconNames.ts";

const svgModules = import.meta.glob<string>("./icons/*.svg", {
	query: "?raw",
	import: "default",
	eager: true,
});

export const icons: Map<IconName, string> = new Map();
for (const [path, svg] of Object.entries(svgModules)) {
	const name = path.replace(/^.*\/(.+)\.svg$/, "$1") as IconName;
	icons.set(name, svg);
}
