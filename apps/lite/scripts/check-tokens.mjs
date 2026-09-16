#!/usr/bin/env node

/**
 * Checks that every CSS custom property Lite reads is one something defines.
 *
 * A `var(--x)` whose name nothing declares doesn't error: the declaration
 * becomes invalid at computed-value time and the property quietly falls back
 * to its fallback, its initial value, or whatever it inherits. That is how
 * `--radius-m` shipped square corners on two panels and `--font-mono` rendered
 * a generic monospace: each name was plausible next to a real token
 * (`--radius-md`, `--text-fontfamily-mono`) and nothing said otherwise.
 *
 * Definitions are gathered from:
 *
 * - @gitbutler/design-core's stylesheets, where the design tokens live.
 * - @pierre/diffs' injected stylesheet. It declares `--diffs-*` on its shadow
 *   host, and Lite's slotted content (conflict actions, custom headers) reads
 *   them by design.
 * - Lite's own CSS: `--x:` declarations and `@function --f(--a, --b)` params.
 * - Lite's TS/TSX: CSS in template strings, and any quoted `"--x"` name,
 *   which is a style key or an argument to setProperty or a local setter.
 *
 * Names in KNOWN_RUNTIME are set by code this script can't see; each entry
 * says who sets it. Add to that list only with the same justification.
 *
 * Usage: node scripts/check-tokens.mjs   (exit 1 on any undefined reference)
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const liteRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const uiSrc = join(liteRoot, "ui", "src");

/** Variables set at runtime by code outside this repo's stylesheets. */
const KNOWN_RUNTIME = new Map([
	// Base UI writes these on its popup positioner element.
	["--anchor-width", "@base-ui-components/react positioner"],
	["--available-height", "@base-ui-components/react positioner"],
	["--transform-origin", "@base-ui-components/react positioner"],
	// Shiki's dual-theme output carries both themes as inline style.
	["--shiki-light", "shiki dual-theme inline style"],
	["--shiki-dark", "shiki dual-theme inline style"],
	// A consumer knob: Logo.module.css sets the default, callers may override.
	["--logo-size", "Logo consumer override"],
]);

/** Stylesheets shipped by dependencies whose tokens Lite reads. */
const DEPENDENCY_STYLES = [
	{
		pkg: "@gitbutler/design-core",
		files: ["core.css", "tokens/tokens.css", "styles/reset.css", "styles/text.css"],
	},
	{ pkg: "@pierre/diffs", files: ["dist/style.js"] },
];

const NAME = "--[a-zA-Z0-9_-]+";
const DECLARATION = new RegExp(`(?<![a-zA-Z0-9_-])(${NAME})\\s*:`, "g");
const FUNCTION_PARAMS = new RegExp(`@function\\s+${NAME}\\s*\\(([^)]*)\\)`, "g");
const QUOTED_NAME = new RegExp(`["'](${NAME})["']`, "g");
const REFERENCE = new RegExp(`var\\(\\s*(${NAME})`, "g");

/**
 * @param {string} dir
 * @returns {Generator<string>}
 */
function* walk(dir) {
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) yield* walk(path);
		else yield path;
	}
}

/**
 * @param {RegExp} regex
 * @param {string} text
 * @param {Set<string>} into
 */
function collect(regex, text, into) {
	for (const match of text.matchAll(regex)) into.add(match[1]);
}

/** @type {Set<string>} */
const defined = new Set();

for (const { pkg, files } of DEPENDENCY_STYLES) {
	const root = join(liteRoot, "node_modules", pkg);
	for (const file of files) collect(DECLARATION, readFileSync(join(root, file), "utf8"), defined);
}

const sources = [...walk(uiSrc)].filter((p) => /\.(css|tsx?)$/.test(p) && statSync(p).isFile());
for (const path of sources) {
	const text = readFileSync(path, "utf8");
	collect(DECLARATION, text, defined);
	if (path.endsWith(".css")) {
		for (const match of text.matchAll(FUNCTION_PARAMS))
			for (const param of match[1].split(",")) defined.add(param.trim());
	} else {
		collect(QUOTED_NAME, text, defined);
	}
}

const known = [...defined];

/**
 * Nearest defined name, for the hint. Levenshtein over a short list is fine.
 * @param {string} name
 * @returns {string | null}
 */
function nearest(name) {
	/** @type {string | null} */
	let best = null;
	let bestDistance = Infinity;
	for (const candidate of known) {
		const d = distance(name, candidate);
		if (d < bestDistance) [best, bestDistance] = [candidate, d];
	}
	return bestDistance <= Math.max(3, name.length / 3) ? best : null;
}

/**
 * @param {string} a
 * @param {string} b
 */
function distance(a, b) {
	const row = Array.from({ length: b.length + 1 }, (_, i) => i);
	for (let i = 1; i <= a.length; i++) {
		let previous = row[0]++;
		for (let j = 1; j <= b.length; j++) {
			const current = row[j];
			row[j] = Math.min(row[j] + 1, row[j - 1] + 1, previous + (a[i - 1] === b[j - 1] ? 0 : 1));
			previous = current;
		}
	}
	return row[b.length];
}

/** @type {string[]} */
const problems = [];
for (const path of sources) {
	const lines = readFileSync(path, "utf8").split("\n");
	lines.forEach((line, index) => {
		for (const match of line.matchAll(REFERENCE)) {
			const name = match[1];
			if (defined.has(name) || KNOWN_RUNTIME.has(name)) continue;
			const hint = nearest(name);
			const where = `${relative(liteRoot, path)}:${index + 1}`;
			const suggestion = hint === null ? "" : `; did you mean ${hint}?`;
			problems.push(`${where}  var(${name}) is not defined anywhere${suggestion}`);
		}
	});
}

if (problems.length > 0) {
	process.stderr.write(
		`${problems.length} undefined custom propert${problems.length === 1 ? "y" : "ies"}:\n\n`,
	);
	for (const problem of problems) process.stderr.write(`  ${problem}\n`);
	process.stderr.write(
		"\nUse a token from @gitbutler/design-core, define the variable, or add it to KNOWN_RUNTIME in scripts/check-tokens.mjs with who sets it.\n",
	);
	process.exit(1);
}

process.stdout.write(`check-tokens: ${sources.length} files, every var(--…) resolves.\n`);
