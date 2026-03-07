#!/usr/bin/env node

/**
 * Optimizes all SVG files in `src/lib/icons/svg/` using svgo,
 * then syncs `src/lib/icons/names.ts`.
 *
 * Usage:
 *   pnpm optimize-icons
 */

import { optimize } from "svgo";
import { readFileSync, writeFileSync, readdirSync } from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const uiRoot = path.resolve(__dirname, "..");
const iconsDir = path.join(uiRoot, "src/lib/icons/svg");

/**
 * Custom svgo plugin: replaces any hardcoded color value on fill/stroke
 * attributes with `currentColor`, leaving `none` untouched.
 *
 * @type {import('svgo').CustomPlugin}
 */
const replaceColorsWithCurrentColor = {
	name: "replaceColorsWithCurrentColor",
	fn() {
		return {
			element: {
				enter(node) {
					for (const attr of ["fill", "stroke"]) {
						const value = node.attributes[attr];
						if (value && value !== "none" && value !== "currentColor") {
							node.attributes[attr] = "currentColor";
						}
					}
					const vectorShapes = ["path", "circle", "ellipse", "rect", "line", "polyline", "polygon"];
					if (vectorShapes.includes(node.name)) {
						node.attributes["vector-effect"] = "non-scaling-stroke";
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
					// Don't collapse groups — our icons use them intentionally
					collapseGroups: false,
					// Keep viewBox — required for proper scaling
					removeViewBox: false,
				},
			},
		},
		// Ensure width/height are set to 100% so icons scale via CSS
		{
			name: "addWidthHeight100Percent",
			fn() {
				return {
					element: {
						enter(node) {
							if (node.name === "svg") {
								node.attributes.width = "100%";
								node.attributes.height = "100%";
							}
						},
					},
				};
			},
		},
		// Replace any hardcoded colors with currentColor
		replaceColorsWithCurrentColor,
	],
};

const iconNamesFile = path.join(uiRoot, "src/lib/icons/names.ts");

function optimizeIcons() {
	const svgFiles = readdirSync(iconsDir).filter((f) => f.endsWith(".svg"));

	let optimized = 0;
	let unchanged = 0;
	let totalSavedBytes = 0;

	for (const file of svgFiles) {
		const filePath = path.join(iconsDir, file);
		const original = readFileSync(filePath, "utf-8");
		const result = optimize(original, { path: filePath, ...svgoConfig });
		const optimizedSvg = result.data;

		if (optimizedSvg !== original) {
			const saved = original.length - optimizedSvg.length;
			totalSavedBytes += saved;
			writeFileSync(filePath, optimizedSvg, "utf-8");
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

function updateIconNames() {
	const svgFiles = readdirSync(iconsDir)
		.filter((f) => f.endsWith(".svg"))
		.map((f) => f.replace(".svg", ""))
		.sort();

	let currentNames = [];
	try {
		const content = readFileSync(iconNamesFile, "utf-8");
		const match = content.match(/\[([^\]]+)\]/s);
		if (match) {
			currentNames = [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
		}
	} catch {
		// File doesn't exist yet
	}

	const svgSet = new Set(svgFiles);
	const currentSet = new Set(currentNames);
	const removed = currentNames.filter((name) => !svgSet.has(name));
	const added = svgFiles.filter((name) => !currentSet.has(name));

	const iconEntries = svgFiles.map((name) => `\t"${name}"`).join(",\n");
	const newContent = `/**
 * Auto-generated icon name list from \`src/lib/icons/svg/*.svg\`.
 * Run \`pnpm optimize-icons\` to regenerate.
 */
export const iconNames = [
${iconEntries},
] as const;

export type IconName = (typeof iconNames)[number];
`;

	const existingContent = (() => {
		try {
			return readFileSync(iconNamesFile, "utf-8");
		} catch {
			return "";
		}
	})();

	if (existingContent === newContent) {
		console.warn("Icon names already in sync — no changes needed.");
		return;
	}

	writeFileSync(iconNamesFile, newContent, "utf-8");

	if (added.length > 0) {
		console.warn("Added:");
		for (const name of added) {
			console.warn(`  + ${name}`);
		}
	}
	if (removed.length > 0) {
		console.warn("Removed:");
		for (const name of removed) {
			console.warn(`  - ${name}`);
		}
	}

	console.warn(`Synced ${svgFiles.length} icons → ${path.relative(uiRoot, iconNamesFile)}`);
}

optimizeIcons();
updateIconNames();
