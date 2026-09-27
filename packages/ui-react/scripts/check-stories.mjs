#!/usr/bin/env node

/**
 * Checks that every component in src/ has a story, and that every stories
 * file opens with Default.
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
 * Every stories file also opens with `Default`: the simplest use of the
 * component, with its controls. The stories after it are examples, so a reader (or an agent
 * reading the manifest) knows the first one is the API and the rest are ways
 * to use it.
 *
 * Usage: node scripts/check-stories.mjs   (exit 1 on any problem)
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

const FIRST_STORY = /^export const (\w+)\s*=/m;

const problems = [
	...components
		.filter((name) => !shown.has(name))
		.map((name) => `src/${name}.tsx has no story. Add src/${name}.stories.tsx.`),
	...files
		.filter((file) => file.endsWith(".stories.tsx"))
		.flatMap((file) => {
			const first = FIRST_STORY.exec(readFileSync(join(src, file), "utf8"))?.[1] ?? "no story";
			return first === "Default"
				? []
				: [
						`src/${file} opens with ${first}. Its first story is Default: the component alone, with its controls.`,
					];
		}),
];

if (problems.length > 0) {
	for (const problem of problems) process.stderr.write(`${problem}\n`);
	process.exit(1);
}

process.stdout.write(
	`check-stories: all ${components.length} components have a story, and every file opens with Default.\n`,
);
