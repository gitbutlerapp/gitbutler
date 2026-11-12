// Figma lets a color variable alias another color and put its own opacity on
// top. tokens-bruecke exports that as { components: "{ref}", alpha: n | "{ref}" },
// which isn't DTCG, and Terrazzo rejects it ("Missing required property
// colorSpace"). So these tokens are lifted out of the JSON before Terrazzo runs
// and written back as color-mix(), which keeps both links: --but-hover follows
// --but-inverse and --but-opacity-bg-hover rather than freezing their values.

import { stripCollectionPrefix } from "./terrazzo-css-helpers.mjs";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

function isComposed(value) {
	return value !== null && typeof value === "object" && typeof value.components === "string";
}

function reference(alias) {
	return `var(${stripCollectionPrefix(alias.slice(1, -1))})`;
}

function toColorMix({ components, alpha }) {
	const amount =
		typeof alpha === "number"
			? `${Number((alpha * 100).toFixed(2))}%`
			: alpha.startsWith("{")
				? reference(alpha)
				: alpha;
	return `color-mix(in srgb, ${reference(components)} ${amount}, transparent)`;
}

/** A color token's value in one mode as CSS: a composed one as color-mix(), an alias as var(). */
function toCss(value) {
	if (isComposed(value)) return toColorMix(value);
	return typeof value === "string" && value.startsWith("{") ? reference(value) : value;
}

/**
 * Writes copies of the token files without their composed colors to `outDir`,
 * for Terrazzo to build, and returns those colors as CSS declarations. A color
 * that aliases a composed one in any mode goes with it, as Terrazzo couldn't
 * resolve the alias: --but-snackbar-muted-foreground is
 * --but-inverse-muted-foreground in light.
 */
export function liftComposedColors(paths, outDir) {
	mkdirSync(outDir, { recursive: true });
	const declarations = [];
	const files = paths.map((path) => ({ path, json: JSON.parse(readFileSync(path, "utf8")) }));
	// The aliases lifted so far, as tokens-bruecke writes them: "{semantic.inverse.muted-foreground}".
	const lifted = new Set();

	function modeValues(token) {
		const modes = token.$extensions?.mode ?? {};
		return [modes.light ?? token.$value, modes.dark ?? token.$value];
	}
	function liftable(token) {
		return modeValues(token).some(
			(value) => isComposed(value) || (typeof value === "string" && lifted.has(value)),
		);
	}
	// Until nothing more goes: a color aliasing a lifted one can itself be aliased.
	for (let more = true; more; ) {
		more = false;
		for (const { json } of files) {
			function walk(node, keys) {
				for (const [key, child] of Object.entries(node)) {
					if (
						(key.startsWith("$") && key !== "$root") ||
						child === null ||
						typeof child !== "object"
					)
						continue;
					if (child.$type === "color" && liftable(child)) {
						const [light, dark] = modeValues(child).map(toCss);
						declarations.push({
							name: stripCollectionPrefix([...keys, key].join(".")),
							value: light === dark ? light : `light-dark(${light}, ${dark})`,
						});
						lifted.add(`{${[...keys, key].join(".")}}`);
						delete node[key];
						more = true;
					} else {
						walk(child, [...keys, key]);
					}
				}
			}
			walk(json, []);
		}
	}
	for (const { path, json } of files)
		writeFileSync(join(outDir, basename(path)), JSON.stringify(json, null, "\t"));

	return declarations;
}
