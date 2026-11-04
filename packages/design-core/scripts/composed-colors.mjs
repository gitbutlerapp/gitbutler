// Figma lets a color variable alias another color and put its own opacity on
// top. tokens-bruecke exports that as { components: "{ref}", alpha: n | "{ref}" },
// which isn't DTCG, and Terrazzo rejects it ("Missing required property
// colorSpace"). So these tokens are lifted out of the JSON before Terrazzo runs
// and written back as color-mix(), which keeps both links: --bg-hover follows
// --fill-gray-bg and --opacity-bg-hover rather than freezing their values.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { stripCollectionPrefix } from "./terrazzo-css-helpers.mjs";

const isComposed = (value) =>
	value !== null &&
	typeof value === "object" &&
	typeof value.components === "string";

const reference = (alias) =>
	`var(${stripCollectionPrefix(alias.slice(1, -1))})`;

function toColorMix({ components, alpha }) {
	const amount =
		typeof alpha === "number"
			? `${Number((alpha * 100).toFixed(2))}%`
			: alpha.startsWith("{")
				? reference(alpha)
				: alpha;
	return `color-mix(in srgb, ${reference(components)} ${amount}, transparent)`;
}

/**
 * Writes copies of the token files without their composed colors to `outDir`,
 * for Terrazzo to build, and returns those colors as CSS declarations.
 */
export function liftComposedColors(paths, outDir) {
	mkdirSync(outDir, { recursive: true });
	const declarations = [];

	for (const path of paths) {
		const json = JSON.parse(readFileSync(path, "utf8"));
		const walk = (node, keys) => {
			for (const [key, child] of Object.entries(node)) {
				if (
					key.startsWith("$") ||
					child === null ||
					typeof child !== "object"
				)
					continue;
				if (child.$type === "color" && isComposed(child.$value)) {
					const modes = child.$extensions?.mode ?? {};
					const light = toColorMix(modes.light ?? child.$value);
					const dark = toColorMix(modes.dark ?? child.$value);
					declarations.push({
						name: stripCollectionPrefix([...keys, key].join(".")),
						value:
							light === dark
								? light
								: `light-dark(${light}, ${dark})`,
					});
					delete node[key];
				} else {
					walk(child, [...keys, key]);
				}
			}
		};
		walk(json, []);
		writeFileSync(
			join(outDir, basename(path)),
			JSON.stringify(json, null, "\t"),
		);
	}

	return declarations;
}
