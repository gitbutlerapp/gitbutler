#!/usr/bin/env node

/**
 * Icon optimization script for apps/lite/ui/src/components/icons.
 *
 * Each transform exists for a concrete reason:
 *
 * - width/height -> "100%". Figma exports fixed pixels (width="16"), but
 *   sizing is owned by CSS: Icon.module.css sets a --icon-size box and the SVG
 *   fills it. A hardcoded width would ignore <Icon size={20} />. viewBox is
 *   kept — that preserves aspect ratio and makes percentage sizing meaningful.
 * - fill/stroke colors -> "currentColor", so one asset works on light and dark
 *   themes and in accent-colored containers. "none" is preserved: it's a
 *   structural value (an unfilled outline shape), not a color.
 * - vector-effect="non-scaling-stroke" on shape elements. Icons are drawn on a
 *   16px grid with 1.5px strokes; without this, a scaled-up icon reads heavier
 *   than its neighbours.
 * - Minification. Icons are inlined into the bundle as raw strings and
 *   injected via dangerouslySetInnerHTML, so Figma's indentation would ship to
 *   the user and land in the DOM. It also gives icons a canonical single-line
 *   form, so a re-export produces a clean diff instead of a whitespace-only one.
 * - iconNames.ts regeneration. IconName is a generated union of the filenames
 *   on disk, which is what makes <Icon name="folder-lock" /> compile-time
 *   checked. Never hand-edit it.
 *
 * This is a text transform, not a geometry pass. It won't clean up what Figma
 * leaves behind — check the export by hand for:
 *
 * - Stray placeholder or mask shapes. A leftover rectangle exported as
 *   fill="#D9D9D9" becomes fill="currentColor" and paints a solid block over
 *   the icon.
 * - Geometry outside the viewBox. Anything beyond "0 0 16 16" is clipped or
 *   overflows unpredictably.
 * - clipPath ids. Figma emits ids like clip0_1800_10322. Several icons are
 *   inlined into the same document, so ids must stay unique — keep Figma's
 *   generated suffix rather than renaming to something generic like "clip0".
 * - Off-grid coordinates. 13.999999 instead of 14 means the frame wasn't
 *   aligned to the pixel grid in Figma; fix it at the source.
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ICONS_DIR = path.resolve(__dirname, "../ui/src/components/icons");
const ICON_NAMES_FILE = path.resolve(__dirname, "../ui/src/components/iconNames.ts");

/**
 * @param {string} message
 */
function writeStdout(message) {
	process.stdout.write(`${message}\n`);
}

/**
 * @param {string} message
 */
function writeStderr(message) {
	process.stderr.write(`${message}\n`);
}

// Colors to preserve as-is (not replaced with currentColor)
const PRESERVE_VALUES = new Set(["none", "currentColor", "currentcolor"]);

/**
 * Replace width/height on the root <svg> element with 100%.
 * @param {string} svg
 * @returns {string}
 */
function normalizeSize(svg) {
	return svg.replace(
		/<svg([^>]*)>/,
		(/** @type {string} */ _match, /** @type {string} */ attrs) => {
			let updated = attrs;
			updated = updated.replace(/\bwidth="[^"]*"/, 'width="100%"');
			updated = updated.replace(/\bheight="[^"]*"/, 'height="100%"');
			// Add if missing
			if (!/\bwidth=/.test(updated)) updated += ' width="100%"';
			if (!/\bheight=/.test(updated)) updated += ' height="100%"';
			return `<svg${updated}>`;
		},
	);
}

/**
 * Replace fill="..." and stroke="..." color values with "currentColor".
 * Preserves "none" and already-correct "currentColor".
 * @param {string} svg
 * @returns {string}
 */
function replaceColors(svg) {
	return svg.replace(
		/\b(fill|stroke)="([^"]*)"/g,
		(/** @type {string} */ _match, /** @type {string} */ attr, /** @type {string} */ value) => {
			const trimmed = value.trim();
			if (PRESERVE_VALUES.has(trimmed.toLowerCase())) return `${attr}="${trimmed}"`;
			// If it looks like a color (hex, rgb, hsl, named color, or a typo like "curentColor")
			if (
				trimmed.startsWith("#") ||
				/^rgb/i.test(trimmed) ||
				/^hsl/i.test(trimmed) ||
				/^[a-zA-Z]+$/.test(trimmed)
			)
				return `${attr}="currentColor"`;
			return `${attr}="${trimmed}"`;
		},
	);
}

/**
 * Add vector-effect="non-scaling-stroke" to all shape/path elements
 * that don't already have it.
 * @param {string} svg
 * @returns {string}
 */
function addNonScalingStroke(svg) {
	const targets = /(<(?:path|circle|ellipse|line|polyline|polygon|rect)\b)([^>]*?)(\/?>)/g;
	return svg.replace(
		targets,
		(
			/** @type {string} */ _match,
			/** @type {string} */ open,
			/** @type {string} */ attrs,
			/** @type {string} */ close,
		) => {
			if (/vector-effect/.test(attrs)) return _match;
			return `${open}${attrs} vector-effect="non-scaling-stroke"${close}`;
		},
	);
}

/**
 * Minify SVG by removing unnecessary whitespace, newlines, and comments.
 * @param {string} svg
 * @returns {string}
 */
function minify(svg) {
	return (
		svg
			// Remove XML comments
			.replace(/<!--[\s\S]*?-->/g, "")
			// Collapse newlines and runs of whitespace into a single space
			.replace(/\s+/g, " ")
			// Remove space between tags
			.replace(/>\s+</g, "><")
			// Remove space before self-closing
			.replace(/\s\/>/g, "/>")
			// Remove space after opening <
			.replace(/<\s+/g, "<")
			// Trim leading/trailing whitespace
			.trim()
	);
}

/**
 * @param {string} svg
 * @returns {string}
 */
function optimizeSvg(svg) {
	let result = svg;
	result = normalizeSize(result);
	result = replaceColors(result);
	result = addNonScalingStroke(result);
	result = minify(result);
	return result;
}

/**
 * @param {string[]} names
 * @returns {string}
 */
function generateIconNamesFile(names) {
	const sorted = [...names].sort((a, b) => a.localeCompare(b));
	const union = sorted.length > 0 ? sorted.map((n) => `"${n}"`).join(" | ") : "never";
	return `// This file is auto-generated by apps/lite/scripts/optimize-icons.mjs.\n// Do not edit this file manually.\n\nexport type IconName = ${union};\n`;
}

/**
 * @param {string} content
 * @returns {Set<string>}
 */
function extractIconNamesFromGeneratedFile(content) {
	const match = content.match(/export\s+type\s+IconName\s*=\s*([\s\S]*?);/);
	if (!match) return new Set();

	const unionBody = match[1].trim();
	if (unionBody === "never" || unionBody === "") return new Set();

	const names = [...unionBody.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
	return new Set(names);
}

/**
 * @param {string[]} iconNames
 * @returns {{added: string[], removed: string[]}}
 */
function diffGeneratedIconNames(iconNames) {
	if (!fs.existsSync(ICON_NAMES_FILE))
		return { added: [...iconNames].sort((a, b) => a.localeCompare(b)), removed: [] };

	const prevTypes = fs.readFileSync(ICON_NAMES_FILE, "utf-8");
	const existingNames = extractIconNamesFromGeneratedFile(prevTypes);
	const nextNames = new Set(iconNames);

	const added = [...nextNames].filter((name) => !existingNames.has(name));
	const removed = [...existingNames].filter((name) => !nextNames.has(name));

	added.sort((a, b) => a.localeCompare(b));
	removed.sort((a, b) => a.localeCompare(b));

	return { added, removed };
}

// ── Main ───────────────────────────────────────────────────────────────

if (!fs.existsSync(ICONS_DIR)) {
	writeStderr(`Icons directory not found: ${ICONS_DIR}`);
	process.exit(1);
}

const files = fs.readdirSync(ICONS_DIR).filter((f) => f.endsWith(".svg"));

let updated = 0;
let unchanged = 0;
const iconNames = [];

for (const file of files) {
	const filePath = path.join(ICONS_DIR, file);
	const name = path.basename(file, ".svg");
	iconNames.push(name);

	const original = fs.readFileSync(filePath, "utf-8");
	const optimized = optimizeSvg(original);

	if (optimized !== original) {
		fs.writeFileSync(filePath, optimized, "utf-8");
		updated++;
		writeStdout(`  ✓ optimized: ${file}`);
	} else {
		unchanged++;
	}
}

// Regenerate iconNames.ts
const { added, removed } = diffGeneratedIconNames(iconNames);
const prevTypes = fs.existsSync(ICON_NAMES_FILE) ? fs.readFileSync(ICON_NAMES_FILE, "utf-8") : "";
const nextTypes = generateIconNamesFile(iconNames);

let typesStatus;
if (prevTypes === nextTypes) typesStatus = "unchanged";
else typesStatus = "updated";

if (typesStatus === "updated") fs.writeFileSync(ICON_NAMES_FILE, nextTypes, "utf-8");

// Stats
writeStdout("");
writeStdout("--- Icon optimization complete ---");
writeStdout(`  Total icons : ${files.length}`);
writeStdout(`  Optimized   : ${updated}`);
writeStdout(`  Unchanged   : ${unchanged}`);
writeStdout(`  iconNames.ts: ${typesStatus}`);

if (added.length > 0 || removed.length > 0) {
	if (added.length > 0) writeStdout(`  Added names : ${added.join(", ")}`);
	if (removed.length > 0) writeStdout(`  Removed names: ${removed.join(", ")}`);
}

if (files.length === 0) writeStdout("  Note        : No SVG files found in the icons directory.");
writeStdout("");
