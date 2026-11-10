// Fails when a preset names a corner or density setting tokens.css doesn't define, so a preset
// can't point an app at an attribute value nothing styles. soft and balanced are Figma's, the
// settings with no block of their own.
//
//   node scripts/check-presets.mjs

import { readFileSync } from "node:fs";

const css = readFileSync("tokens/tokens.css", "utf8");
const presets = JSON.parse(readFileSync("presets.json", "utf8"));

function settings(attribute, figma) {
	const generated = [...css.matchAll(new RegExp(`:root\\[data-but-${attribute}="(\\w+)"\\]`, "g"))];
	return new Set([figma, ...generated.map(([, value]) => value)]);
}
const known = { radius: settings("radius", "soft"), density: settings("density", "balanced") };

const problems = [];
const ids = new Set();
for (const preset of presets) {
	if (ids.has(preset.id)) problems.push(`${preset.id}: the id is taken`);
	ids.add(preset.id);
	for (const [setting, values] of Object.entries(known)) {
		if (!values.has(preset[setting]))
			problems.push(
				`${preset.id}: ${setting} "${preset[setting]}" isn't one of ${[...values].join(", ")}`,
			);
	}
}

if (problems.length) {
	process.stderr.write(`check-presets: ${problems.length} problems:\n${problems.join("\n")}\n`);
	process.exit(1);
}
process.stdout.write(
	`check-presets: all ${presets.length} presets name settings tokens.css defines.\n`,
);
