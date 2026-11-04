// Fails when the hand-written stylesheets read a custom property that neither
// tokens.css nor the stylesheets themselves declare. A token renamed in Figma
// otherwise leaves them pointing at nothing, and the browser silently drops
// the declaration: 6.0.0 shipped text.css reading --text-fontfamily-base, and
// every app fell back to the default serif.
//
//   node scripts/check-style-vars.mjs

import { readFileSync } from "node:fs";

const STYLESHEETS = ["styles/reset.css", "styles/text.css", "core.css"];

const declared = new Set();
for (const file of ["tokens/tokens.css", ...STYLESHEETS]) {
	for (const [, name] of readFileSync(file, "utf8").matchAll(
		/(--[\w-]+)\s*:/g,
	)) {
		declared.add(name);
	}
}

const missing = [];
for (const file of STYLESHEETS) {
	const lines = readFileSync(file, "utf8").split("\n");
	lines.forEach((line, i) => {
		for (const [, name] of line.matchAll(/var\(\s*(--[\w-]+)/g)) {
			if (!declared.has(name)) missing.push(`${file}:${i + 1}  ${name}`);
		}
	});
}

if (missing.length) {
	console.error(
		`These stylesheets read custom properties nothing declares:\n  ${missing.join("\n  ")}`,
	);
	process.exit(1);
}
console.log(
	`check-style-vars: every var(--…) in ${STYLESHEETS.join(", ")} resolves.`,
);
