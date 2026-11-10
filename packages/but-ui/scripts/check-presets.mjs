// Fails when a preset names a corner or density setting the build doesn't define, so a preset
// can't point an app at an attribute value nothing styles. tokens/settings.json has every
// setting, Figma's included.
//
//   node scripts/check-presets.mjs

import { readFileSync } from "node:fs";

const settings = JSON.parse(readFileSync("tokens/settings.json", "utf8"));
const presets = JSON.parse(readFileSync("presets.json", "utf8"));

const problems = [];
const ids = new Set();
for (const preset of presets) {
	if (ids.has(preset.id)) problems.push(`${preset.id}: the id is taken`);
	ids.add(preset.id);
	for (const setting of ["radius", "density"]) {
		const choices = Object.keys(settings[setting].values);
		if (!choices.includes(preset[setting]))
			problems.push(
				`${preset.id}: ${setting} "${preset[setting]}" isn't one of ${choices.join(", ")}`,
			);
	}
}

if (problems.length) {
	process.stderr.write(`check-presets: ${problems.length} problems:\n${problems.join("\n")}\n`);
	process.exit(1);
}
process.stdout.write(
	`check-presets: all ${presets.length} presets name settings the build defines.\n`,
);
