import { cssVarName, stripCollectionPrefix } from "./terrazzo-css-helpers.mjs";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const FX_TOKENS_PATH = resolve("tokens/json/fx.tokens.json");

function resolveTokenRef(ref) {
	// Converts "{semantic.shadow}" -> "var(--but-shadow)"
	const inner = ref.replace(/^\{|\}$/g, "");
	return `var(${stripCollectionPrefix(inner) ?? cssVarName(inner)})`;
}

function formatDimension(dim) {
	if (typeof dim === "string") return dim;
	if (typeof dim === "number") return dim === 0 ? "0" : `${dim}px`;
	const { value, unit } = dim;
	return value === 0 ? "0" : `${value}${unit}`;
}

function buildShadowValue(layers) {
	return layers
		.map(({ inset, color, offsetX, offsetY, blur, spread }) => {
			const colorValue = color.startsWith("{") ? resolveTokenRef(color) : color;
			const parts = [offsetX, offsetY, blur, spread].map(formatDimension);
			parts.push(colorValue);
			if (inset) parts.unshift("inset");
			return parts.join(" ");
		})
		.join(", ");
}

// Walks the fx collection and yields every shadow token, wherever it sits in
// the tree. Figma effect styles are grouped freely ("shadow/sm", "popup"), so
// anything but a recursive walk silently drops tokens from the build.
function collectShadowTokens(node, path = [], out = []) {
	if (!node || typeof node !== "object") return out;
	if (node.$type === "shadow") {
		out.push({ path, token: node });
		return out;
	}
	for (const [key, child] of Object.entries(node)) {
		if (key.startsWith("$")) continue;
		collectShadowTokens(child, [...path, key], out);
	}
	return out;
}

// "shadow.sm" -> --but-shadow-sm, "popup" -> --but-shadow-popup: every effect
// style lives in the --but-shadow-* namespace regardless of how it is grouped
// in Figma. (--but-shadow itself is the shadow's colour.)
function shadowVarName(path) {
	const name = path.join("-");
	return cssVarName(name.startsWith("shadow-") ? name : `shadow-${name}`);
}

export function generateShadowVars(css) {
	const fxTokens = JSON.parse(readFileSync(FX_TOKENS_PATH, "utf8"));
	const shadows = collectShadowTokens(fxTokens?.fx ?? {});
	const varNames = shadows.map(({ path }) => shadowVarName(path));
	const vars = shadows.map(
		({ token }, i) => `  ${varNames[i]}: ${buildShadowValue(token.$value)};`,
	);

	if (vars.length === 0) return css;

	// Strip any previously injected shadow shorthand vars and the marker comment.
	const stripPattern = new RegExp(
		`^  (?:/\\* shadow vars \\*/|(?:${varNames.map((v) => v.replace(/-/g, "\\-")).join("|")}):.*)\n`,
		"gm",
	);
	const cleaned = css.replace(stripPattern, "");

	// Inject before the first closing brace of :root.
	const marker = "  /* shadow vars */";
	const block = `${marker}\n${vars.join("\n")}`;
	return cleaned.replace(/(\n})(\n\n:root\.light)/, `\n${block}\n}$2`);
}
