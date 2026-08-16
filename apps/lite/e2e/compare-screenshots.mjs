// Compares two screenshot runs, stages the pairs that actually changed, and
// renders the markdown section for a pull request.
//
//   node compare-screenshots.mjs <beforeDir> <afterDir> <publishDir> <baseUrl> <sectionFile>
//
// Surfaces are captured unconditionally, so "which ones matter" is decided here:
// identical fixtures and viewport mean an untouched surface is byte-identical.
// The rendered section always states the denominator, because a broken run and a
// run with no visual changes both produce zero differing pairs.

import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const beforeDir = args[0] ?? "";
const afterDir = args[1] ?? "";
const publishDir = args[2] ?? "";
const baseUrl = args[3] ?? "";
const sectionFile = args[4] ?? "";
if (sectionFile === "") {
	process.stderr.write(
		"usage: compare-screenshots.mjs <beforeDir> <afterDir> <publishDir> <baseUrl> <sectionFile>\n",
	);
	process.exit(1);
}

/** @param {string} file */
const digest = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");

/** @param {string} dir */
const pngs = (dir) => readdirSync(dir).filter((name) => name.endsWith(".png"));

const afterShots = pngs(afterDir);
const beforeShots = new Set(pngs(beforeDir));

const changed = [];
const unchanged = [];
const added = [];

for (const name of afterShots.sort()) {
	const surface = path.basename(name, ".png");
	if (!beforeShots.has(name)) {
		// A surface the catalogue gained, or one the base could not render.
		added.push(surface);
		continue;
	}
	if (digest(path.join(beforeDir, name)) === digest(path.join(afterDir, name))) {
		unchanged.push(surface);
		continue;
	}
	changed.push(surface);
}

for (const phase of ["before", "after"])
	mkdirSync(path.join(publishDir, phase), { recursive: true });
for (const surface of [...changed, ...added]) {
	const name = `${surface}.png`;
	if (beforeShots.has(name))
		copyFileSync(path.join(beforeDir, name), path.join(publishDir, "before", name));
	copyFileSync(path.join(afterDir, name), path.join(publishDir, "after", name));
}

const total = afterShots.length;
/**
 * @param {string} phase
 * @param {string} surface
 */
const img = (phase, surface) => `<img src="${baseUrl}/${phase}/${surface}.png" width="420">`;

const lines = ["<!-- lite-screenshots -->", "## 📸 UI screenshots", ""];

if (changed.length === 0 && added.length === 0) {
	lines.push(
		`Captured ${total} surfaces; none of them changed.`,
		"",
		"If you expected a visual difference here, treat this as a failed run rather than",
		"a clean bill of health — the two runs may have built the same code.",
	);
} else {
	lines.push(
		`Captured ${total} surfaces, ${changed.length + added.length} changed.`,
		"",
		"Both runs use the same seeded fixtures and viewport, so surfaces missing below are byte-identical.",
		"",
	);
	for (const surface of changed) {
		lines.push(
			`### ${surface}`,
			"",
			"| Before | After |",
			"| --- | --- |",
			`| ${img("before", surface)} | ${img("after", surface)} |`,
			"",
		);
	}
	for (const surface of added) {
		const caption = "New surface — no baseline to compare against.";
		lines.push(`### ${surface}`, "", caption, "", img("after", surface), "");
	}
	if (unchanged.length > 0) {
		lines.push(
			"<details>",
			`<summary>${unchanged.length} surfaces unchanged</summary>`,
			"",
			unchanged.map((surface) => `\`${surface}\``).join(", "),
			"",
			"</details>",
			"",
		);
	}
}

writeFileSync(sectionFile, `${lines.join("\n")}\n`);
process.stdout.write(`total=${total}\nchanged=${changed.length}\nadded=${added.length}\n`);
