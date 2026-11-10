// Fails when Figma's ramps and the generated ones disagree. generate-ramps.mjs makes every ramp
// into tokens/ramps.json, they go into 🍑 But UI's ramps collection, and the tokens are exported
// from there: an edit in Figma, a ramp left out of it, or one Figma has that the generator
// doesn't, would otherwise go unnoticed.
//
//   node scripts/check-ramps.mjs

import { readFileSync } from "node:fs";

const generated = JSON.parse(readFileSync("tokens/ramps.json", "utf8"));
const exported = JSON.parse(readFileSync("tokens/json/ramps.tokens.json", "utf8")).ramps ?? {};

const hues = (ramps) => Object.keys(ramps).filter((key) => !key.startsWith("$"));
const problems = [];
for (const hue of hues(generated))
	if (!(hue in exported)) problems.push(`${hue}: generated, but not in Figma`);
for (const hue of hues(exported))
	if (!(hue in generated)) problems.push(`${hue}: in Figma, but not generated`);
for (const hue of hues(generated).filter((hue) => hue in exported)) {
	for (const mode of ["light", "dark"]) {
		for (const [step, want] of Object.entries(generated[hue][mode])) {
			const got = exported[hue][step]?.$extensions?.mode?.[mode];
			if (got?.toLowerCase() !== want.toLowerCase())
				problems.push(
					`${hue}-${step} ${mode}: Figma has ${got ?? "nothing"}, the generator ${want}`,
				);
		}
	}
}

if (problems.length) {
	process.stderr.write(
		`check-ramps: Figma's ramps and tokens/ramps.json disagree. Put the generated ones into Figma and export, or fix the generator:\n${problems.join("\n")}\n`,
	);
	process.exit(1);
}
process.stdout.write(
	`check-ramps: all ${hues(generated).length} ramps in Figma match the generated ones, every step in light and dark.\n`,
);
