#!/usr/bin/env node

/**
 * Checks that every component in src/ has a story.
 *
 * A component without a story has no visual check (Chromatic only sees
 * stories) and is missing from Storybook's component manifest, which is how
 * an agent finds what the library offers. An agent that can't find a
 * component builds a one-off in feature CSS instead.
 *
 * A component is a PascalCase `.tsx` file directly in src/. It counts as
 * having a story when some `*.stories.tsx` in src/ imports it, so a component
 * shown inside another's story (FileIcon in List's) passes too.
 *
 * Usage: node scripts/check-stories.mjs   (exit 1 on any component without a story)
 */

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const src = join(dirname(fileURLToPath(import.meta.url)), "..", "src");

const COMPONENT = /^([A-Z][A-Za-z0-9]*)\.tsx$/;
const LOCAL_IMPORT = /from\s+["']\.\/([A-Z][A-Za-z0-9]*)(?:\.tsx)?["']/g;

const files = readdirSync(src);

const components = files
	.map((file) => COMPONENT.exec(file)?.[1])
	.filter((name) => name !== undefined);

const shown = new Set();
for (const file of files) {
	if (!file.endsWith(".stories.tsx")) continue;
	for (const [, name] of readFileSync(join(src, file), "utf8").matchAll(LOCAL_IMPORT))
		shown.add(name);
}

const problems = components
	.filter((name) => !shown.has(name))
	.map((name) => `src/${name}.tsx has no story. Add src/${name}.stories.tsx.`);

if (problems.length > 0) {
	for (const problem of problems) process.stderr.write(`${problem}\n`);
	process.exit(1);
}

process.stdout.write(`check-stories: all ${components.length} components have a story.\n`);
