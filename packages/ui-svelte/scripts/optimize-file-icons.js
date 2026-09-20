#!/usr/bin/env node
/**
 * optimize-file-icons.js
 *
 * Two modes:
 *
 * 1. Optimize in place (no argument):
 *    Reads all SVG files from `src/lib/components/file/icon/svg/`, optimises
 *    them with svgo, and replaces hardcoded hex colours with CSS variables.
 *
 * 2. Import from a source directory:
 *    Reads SVG files from <svg-dir>, optimises them, and writes each one into
 *    `src/lib/components/file/icon/svg/` (overwriting existing, adding new).
 *
 * Usage:
 *   node scripts/optimize-file-icons.js           # optimize in place
 *   node scripts/optimize-file-icons.js <svg-dir> # import + optimize
 */

import { optimize } from "svgo";
import { readFileSync, writeFileSync, readdirSync } from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const SVG_OUT_DIR = path.join(ROOT, "src/lib/components/file/icon/svg");

// ── 1. Colour map – canonical hex values from CSS variables → CSS variable ───
const COLOR_MAP = {
	"#807976": "var(--file-icon-gray, currentColor)",
	"#16a34a": "var(--file-icon-green, currentColor)",
	"#14b8a6": "var(--file-icon-teal, currentColor)",
	"#0ea5e9": "var(--file-icon-blue, currentColor)",
	"#0e7ce9": "var(--file-icon-dark-blue, currentColor)",
	"#eab308": "var(--file-icon-yellow, currentColor)",
	"#f5700b": "var(--file-icon-orange, currentColor)",
	"#ef4444": "var(--file-icon-red, currentColor)",
	"#f472b6": "var(--file-icon-pink, currentColor)",
	"#a855f7": "var(--file-icon-purple, currentColor)",
	"#5249f8": "var(--file-icon-violet, currentColor)",
};

/**
 * Custom svgo plugin: replaces hardcoded hex colours on fill/stroke
 * attributes with the matching CSS variable from COLOR_MAP.
 *
 * @type {import('svgo').CustomPlugin}
 */
const replaceColorsWithCssVars = {
	name: "replaceColorsWithCssVars",
	fn() {
		return {
			element: {
				enter(node) {
					for (const attr of ["fill", "stroke"]) {
						const value = node.attributes[attr];
						if (value) {
							const cssVar = COLOR_MAP[value.toLowerCase()];
							if (cssVar) node.attributes[attr] = cssVar;
						}
					}
				},
			},
		};
	},
};

/** @type {import('svgo').Config} */
const svgoConfig = {
	plugins: [
		{
			name: "preset-default",
			params: {
				overrides: {
					collapseGroups: false,
					removeViewBox: false,
				},
			},
		},
		// Remove width/height so icons scale via CSS
		{
			name: "removeWidthHeight",
			fn() {
				return {
					element: {
						enter(node) {
							if (node.name === "svg") {
								delete node.attributes.width;
								delete node.attributes.height;
							}
						},
					},
				};
			},
		},
		replaceColorsWithCssVars,
	],
};

// ── 2. Optimise a single SVG file ────────────────────────────────────────────
function processFile(filePath) {
	const original = readFileSync(filePath, "utf-8");
	const result = optimize(original, { path: filePath, ...svgoConfig });
	return { original, optimized: result.data };
}

// ── 3. Main ──────────────────────────────────────────────────────────────────
const svgDir = process.argv[2];

if (svgDir) {
	// Import mode: read from <svg-dir>, write into SVG_OUT_DIR
	const svgFiles = readdirSync(svgDir).filter((f) => f.endsWith(".svg"));
	let added = 0;
	let updated = 0;

	for (const file of svgFiles) {
		const srcPath = path.join(svgDir, file);
		const destPath = path.join(SVG_OUT_DIR, file);
		const { optimized } = processFile(srcPath);

		let existing = null;
		try {
			existing = readFileSync(destPath, "utf-8");
		} catch {
			// new file
		}

		writeFileSync(destPath, optimized, "utf-8");
		if (existing === null) {
			console.warn(`+ added    ${file}`);
			added++;
		} else if (existing !== optimized) {
			console.warn(`✓ updated  ${file}`);
			updated++;
		}
	}

	console.warn(`\nDone. ${added} added, ${updated} updated.`);
} else {
	// Optimize-in-place mode: process all existing SVGs in SVG_OUT_DIR
	const svgFiles = readdirSync(SVG_OUT_DIR).filter((f) => f.endsWith(".svg"));
	let optimized = 0;
	let unchanged = 0;
	let totalSavedBytes = 0;

	for (const file of svgFiles) {
		const filePath = path.join(SVG_OUT_DIR, file);
		const { original, optimized: result } = processFile(filePath);

		if (result !== original) {
			const saved = original.length - result.length;
			totalSavedBytes += saved;
			writeFileSync(filePath, result, "utf-8");
			optimized++;
		} else {
			unchanged++;
		}
	}

	console.warn(`Optimized ${optimized} icon(s), ${unchanged} already optimal.`);
	if (totalSavedBytes > 0) {
		console.warn(`Saved ${(totalSavedBytes / 1024).toFixed(2)} KB total.`);
	}
}
