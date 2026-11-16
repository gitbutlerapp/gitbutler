// Shots of the GPUI client's examples of this library's stories, for the docs site's GPUI view:
// every `id: "components-…--…"` in crates/but-gpui/src/stories, light and dark, at 2x, into
// src/gpui as `<story id>.<appearance>.png`.
//
//   node scripts/gpui-shots.mjs [story id…]
//
// Each opens the stories window on that example alone, fitted to it, through
// crates/but-gpui/scripts/drive.sh, so it builds the client with `dev-script` and never shows a
// window.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");
const drive = join(root, "crates/but-gpui/scripts/drive.sh");
const stories = join(root, "crates/but-gpui/src/stories");
const out = resolve(here, "../src/gpui");
mkdirSync(out, { recursive: true });

const EXAMPLE_ID = /id: "(components-[a-z0-9-]+--[a-z0-9-]+)"/g;
const all = readdirSync(stories)
	.filter((file) => file.endsWith(".rs"))
	.flatMap((file) =>
		[...readFileSync(join(stories, file), "utf8").matchAll(EXAMPLE_ID)].map(([, id]) => id),
	);
const asked = process.argv.slice(2);
const ids = asked.length > 0 ? all.filter((id) => asked.includes(id)) : all;

const scratch = mkdtempSync(join(tmpdir(), "gpui-shots-"));
for (const id of ids) {
	for (const appearance of ["light", "dark"]) {
		const shot = join(out, `${id}.${appearance}.png`);
		// The window fits the example a frame after it opens. A region keeps the shot at the
		// display's scale, and one past the window's edges is cut to them: the whole window.
		const script = join(scratch, `${id}.${appearance}`);
		writeFileSync(script, `wait 400\nshot ${shot} 0 0 10000 10000\n`);
		process.stdout.write(`${id} ${appearance}\n`);
		execFileSync(drive, ["run", "--stories", "--timeout", "120", script], {
			stdio: ["ignore", "ignore", "inherit"],
			env: { ...process.env, GITBUTLER_GPUI_STORY: id, GITBUTLER_GPUI_APPEARANCE: appearance },
		});
	}
}
process.stdout.write(`${ids.length * 2} shots in ${out}\n`);
