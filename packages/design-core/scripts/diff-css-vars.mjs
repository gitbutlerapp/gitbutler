// Compares the CSS custom properties of two tokens.css files and reports what
// changed, plus the semver bump that change implies. Consumers reference these
// vars by name, so removals and renames are breaking.
//
//   node scripts/diff-css-vars.mjs <old.css> <new.css> [--json]

import { readFileSync } from "node:fs";

function parseVars(file) {
	let css;
	try {
		css = readFileSync(file, "utf8");
	} catch {
		return new Map(); // no previous file: everything counts as added
	}
	const vars = new Map();
	// --name: value;  — values may span lines (light-dark(...), shadows)
	const re = /(--[\w-]+)\s*:\s*([^;]+);/g;
	let match;
	while ((match = re.exec(css)) !== null) {
		vars.set(match[1], match[2].replace(/\s+/g, " ").trim());
	}
	return vars;
}

const [oldFile, newFile] = process.argv.slice(2);
const before = parseVars(oldFile);
const after = parseVars(newFile);

const added = [...after.keys()].filter((name) => !before.has(name)).sort();
const removed = [...before.keys()].filter((name) => !after.has(name)).sort();
const changed = [...after.keys()]
	.filter((name) => before.has(name) && before.get(name) !== after.get(name))
	.sort()
	.map((name) => ({ name, from: before.get(name), to: after.get(name) }));

const bump = removed.length ? "major" : added.length ? "minor" : changed.length ? "patch" : "none";

const result = { bump, added, removed, changed };

if (process.argv.includes("--json")) {
	console.log(JSON.stringify(result));
} else {
	const lines = [];
	if (removed.length) lines.push(`### Removed (breaking) — ${removed.length}`, ...removed.map((n) => `- \`${n}\``));
	if (added.length) lines.push(`### Added — ${added.length}`, ...added.map((n) => `- \`${n}\``));
	if (changed.length)
		lines.push(
			`### Changed — ${changed.length}`,
			"",
			"| Token | Before | After |",
			"| --- | --- | --- |",
			...changed.map((c) => `| \`${c.name}\` | \`${c.from}\` | \`${c.to}\` |`),
		);
	console.log(lines.length ? lines.join("\n") : "_No CSS custom properties changed._");
}
