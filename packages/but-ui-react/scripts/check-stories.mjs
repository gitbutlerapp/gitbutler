#!/usr/bin/env node

/**
 * Checks that every component in src/ has a story, and that every stories
 * file opens with Default.
 *
 * A component without a story is missing from the docs site and from the
 * story tests, and an agent that can't find a component builds a one-off in
 * feature CSS instead.
 *
 * A component is a PascalCase `.tsx` file directly in src/. It counts as
 * having a story when some `*.stories.tsx` in src/ imports it, so a component
 * shown inside another's story (FileIcon in List's) passes too.
 *
 * Every stories file also opens with `Default`: the simplest use of the
 * component. The stories after it are examples, so a reader knows the first
 * one is the API and the rest are ways to use it.
 *
 * The GPUI client draws some of these stories too (`Example`s in crates/but-gpui/src/stories),
 * under the story's id and name, so the docs site can show the two side by side. Each has to name
 * a story here as the site does, or its shot never shows.
 *
 * Usage: node scripts/check-stories.mjs   (exit 1 on any problem)
 */

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const src = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const gpuiStories = join(src, "..", "..", "..", "crates", "but-gpui", "src", "stories");

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

// As stories/stories.ts names a story: `IconOnly` → `Icon Only`, at `components-button--icon-only`.
/** @param {string} exportName */
const storyName = (exportName) =>
	exportName.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/([A-Z])([A-Z][a-z])/g, "$1 $2");
const PAGE_ID = /\bid: "([a-z0-9-]+)"/;
const STORY = /^export const (\w+)\s*=\s*meta\.story\(/gm;
/** @type {Map<string, string>} */
const storyNames = new Map();
for (const file of files) {
	if (!file.endsWith(".stories.tsx")) continue;
	const source = readFileSync(join(src, file), "utf8");
	const page = PAGE_ID.exec(source)?.[1];
	if (page === undefined) continue;
	for (const [, exportName] of source.matchAll(STORY)) {
		const name = storyName(exportName);
		storyNames.set(`${page}--${name.toLowerCase().replaceAll(" ", "-")}`, name);
	}
}

const EXAMPLE = /id: "([^"]+)",\s*name: "([^"]+)"/g;
const gpuiExamples = readdirSync(gpuiStories)
	.filter((file) => file.endsWith(".rs"))
	.flatMap((file) =>
		[...readFileSync(join(gpuiStories, file), "utf8").matchAll(EXAMPLE)].map((match) => ({
			file,
			id: match[1],
			name: match[2],
		})),
	);

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
				: [`src/${file} opens with ${first}. Its first story is Default: the component alone.`];
		}),
	...gpuiExamples.flatMap(({ file, id, name }) => {
		const story = storyNames.get(id);
		if (story === undefined)
			return [`crates/but-gpui/src/stories/${file} draws ${id}, which no story here has.`];
		return story === name
			? []
			: [
					`crates/but-gpui/src/stories/${file} names ${id} "${name}"; the docs site says "${story}".`,
				];
	}),
];

if (problems.length > 0) {
	for (const problem of problems) process.stderr.write(`${problem}\n`);
	process.exit(1);
}

process.stdout.write(
	`check-stories: all ${components.length} components have a story, every file opens with Default, and GPUI's ${gpuiExamples.length} examples name stories here.\n`,
);
