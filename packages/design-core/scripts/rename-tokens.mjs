// Moves code onto the but-ui names: every design-core name in token-map.json is
// rewritten to its --but-* replacement, in CSS, TypeScript, Svelte and Markdown,
// declarations and comments included. Run it on the folders to move:
//
//   node packages/design-core/scripts/rename-tokens.mjs packages/ui-react/src
//
// Three old names map to more than one new one by where they sit (token-map.json's
// "review"); they get the default and are listed, so a person can pick card,
// popover or selected where those fit. Pass --but-dev to also map but.dev's own
// aliases (--pop, --tint …).

import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const map = JSON.parse(readFileSync(join(here, "../token-map.json"), "utf8"));
const args = process.argv.slice(2);
const names = {
	...map.colour,
	...map.prefix,
	...(args.includes("--but-dev") ? map.butdev_local : {}),
};
const roots = args.filter((a) => !a.startsWith("--"));
const EXTENSIONS = new Set([".css", ".ts", ".tsx", ".js", ".mjs", ".svelte", ".md", ".svg"]);
const SKIP = new Set(["node_modules", "dist", "build", ".svelte-kit", "json"]);

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const pattern = new RegExp(
	`(?<![\\w-])(${Object.keys(names)
		.sort((a, b) => b.length - a.length)
		.map(escape)
		.join("|")})(?![\\w-])`,
	"g",
);

function* files(path) {
	if (statSync(path).isDirectory()) {
		for (const entry of readdirSync(path)) if (!SKIP.has(entry)) yield* files(join(path, entry));
	} else if (
		EXTENSIONS.has(extname(path)) &&
		!path.endsWith(".d.ts") &&
		!path.endsWith("tokens.css")
	) {
		yield path;
	}
}

let total = 0;
const review = [];
for (const root of roots) {
	for (const file of files(resolve(root))) {
		const text = readFileSync(file, "utf8");
		let count = 0;
		const out = text.replace(pattern, (old, _name, offset) => {
			count++;
			if (map.review[old]) {
				const line = text.slice(0, offset).split("\n").length;
				review.push(
					`${relative(process.cwd(), file)}:${line}  ${old} → ${names[old]}  (or ${map.review[old]})`,
				);
			}
			return names[old];
		});
		if (count) {
			writeFileSync(file, out);
			total += count;
			console.log(`${String(count).padStart(4)}  ${relative(process.cwd(), file)}`);
		}
	}
}
console.log(`\n${total} names rewritten.`);
if (review.length)
	console.log(`\nGiven the default; check where another fits:\n${review.join("\n")}`);
