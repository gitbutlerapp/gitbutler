#!/usr/bin/env node

/**
 * Writes `figma-descriptions.json`: the description each component's drawing in 🍑 But UI
 * carries, made from the code so the two can't say different things.
 *
 * A component's drawing is the Figma node its stories link in their meta's `design` parameter,
 * else in their `Default` story's. Its description is the component's JSDoc, then its `@figma`
 * tag (what only the drawing has, such as a `state` property for what CSS draws under the
 * pointer), then a `Rules:` line linking each note its `@see` tags name, and a `Code:` line with
 * its import. The JSDoc says what the component is and how to pick between its props; the rules
 * for using it are the notes', so the description links them rather than repeating them.
 *
 * Only the components in MOVED are written, as each one's words move out of Figma into its JSDoc
 * and notes; the rest keep the description written in Figma until theirs move too.
 *
 * The file is applied to Figma through the Figma MCP's `use_figma` (the REST API can't write a
 * description); AGENTS.md says how.
 *
 *   node scripts/figma-descriptions.mjs          writes figma-descriptions.json
 *   node scripts/figma-descriptions.mjs --check  fails if it's out of date or an @see is missing,
 *                                                and, with FIGMA_API_KEY and FIGMA_FILE_KEY set,
 *                                                if a drawing's description differs from it
 */

import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(packageRoot, "../..");
const src = join(packageRoot, "src");
const out = join(packageRoot, "figma-descriptions.json");
const blob = "https://github.com/gitbutlerapp/gitbutler/blob/master/";

/**
 * The JSDoc right before `export … name` in `source`: its text, and its tags as [tag, value].
 * @param {string} source
 * @param {string} name
 * @returns {{ text: string, tags: [string, string][] } | undefined}
 */
function jsdocOf(source, name) {
	const comment = new RegExp(
		String.raw`(\/\*\*(?:(?!\*\/)[\s\S])*\*\/)\s*export (?:const|function|class) ${name}\b`,
	).exec(source)?.[1];
	if (comment === undefined) return undefined;
	const body = comment
		.split("\n")
		.map((line) => line.replace(/^\s*\/?\*+\/?\s?/, "").replace(/\s*\*\/$/, ""))
		.join("\n");
	const [text = "", ...rest] = body.split(/(?:^|\n)(?=@\w)/);
	/** @type {[string, string][]} */
	const tags = rest.map((tag) => {
		const [, key = "", value = ""] = /^@(\w+)\s*([\s\S]*)$/.exec(tag.trim()) ?? [];
		return [key, value.trim()];
	});
	return { text: text.trim(), tags };
}

/**
 * The documented members of `export type <name>Variant` in `source`, as Figma lists them: one
 * `- value: what it's for` line each.
 * @param {string} source
 * @param {string} name
 * @returns {string}
 */
function variantsOf(source, name) {
	const type = new RegExp(String.raw`export type ${name}Variant =([\s\S]*?);`).exec(source)?.[1];
	if (type === undefined) return "";
	return [...type.matchAll(/\/\*\*([\s\S]*?)\*\/\s*\|\s*"([\w-]+)"/g)]
		.map(([, doc = "", value = ""]) => {
			const words = doc
				.replaceAll(/\n\s*\*\s?/g, " ")
				.replaceAll(/\s+/g, " ")
				.trim();
			return `- ${value}: ${words}`;
		})
		.join("\n");
}

/**
 * `text` as written, from Figma's description, which it hands back with `'`, `"`, `<`, `>` and `&`
 * as HTML entities.
 * @param {string} text
 * @returns {string}
 */
function unescaped(text) {
	return text
		.replaceAll("&#39;", "'")
		.replaceAll("&quot;", '"')
		.replaceAll("&lt;", "<")
		.replaceAll("&gt;", ">")
		.replaceAll("&amp;", "&");
}

/** A node id in Figma's link form, `217-317`, or as a helper is handed it, `figma("217-317")`. */
const NODE = /(?:node-id=|\(")(\d+)-(\d+)/;

/** The components whose description is written from the code; see the top of this file. */
const MOVED = new Set(["Badge", "Banner", "Button", "MetaPart", "Snackbar", "Toasts"]);

/**
 * `text` as Figma shows it: a `{@link Name}` as the name alone, and a paragraph on one line, as
 * Figma wraps it.
 * @param {string} text
 * @returns {string}
 */
function plain(text) {
	return text
		.replaceAll(/\{@link\s+([^}\s]+)\s*\}/g, "$1")
		.replaceAll(/([^\n])\n(?!\n|- )/g, "$1 ");
}

/**
 * The node a stories file links as its component's drawing: the one in its `stories({…})` meta,
 * else its `Default` story's.
 * @param {string} stories
 * @returns {RegExpExecArray | null}
 */
function drawing(stories) {
	const meta = stories.split(/\bstories(?:<[^>]*>)?\(\{/)[1]?.split(/\n\}\);/)[0] ?? "";
	const defaultStory =
		stories.split(/export const Default\b/)[1]?.split(/\nexport const /)[0] ?? "";
	return NODE.exec(meta) ?? NODE.exec(defaultStory);
}

/** @type {Record<string, { component: string, file: string, description: string }>} */
const descriptions = {};
/** @type {string[]} */
const problems = [];
for (const storiesFile of readdirSync(src)
	.filter((file) => file.endsWith(".stories.tsx"))
	.sort()) {
	const stories = readFileSync(join(src, storiesFile), "utf8");
	const component = /\bstories(?:<[^>]*>)?\(\{[\s\S]*?\bcomponent: (\w+)/.exec(stories)?.[1];
	const node = drawing(stories);
	if (component === undefined || node === null) continue;
	const file = new RegExp(
		String.raw`import \{[^}]*\b${component}\b[^}]*\} from "\./(\w+\.tsx)"`,
	).exec(stories)?.[1];
	if (file === undefined) continue;
	const source = readFileSync(join(src, file), "utf8");
	const doc = jsdocOf(source, component);
	const variants = variantsOf(source, component);
	/** @param {string} name */
	const tagged = (name) =>
		(doc?.tags ?? []).filter(([tag]) => tag === name).map(([, value]) => value);
	const notes = tagged("see");
	const figma = tagged("figma").join("\n\n");
	for (const note of notes)
		if (!existsSync(join(repoRoot, note))) problems.push(`${file}: @see ${note}: no such file`);
	const lines = [
		plain(doc?.text ?? ""),
		variants === "" ? "" : `variant:\n${plain(variants)}`,
		figma === "" ? "" : `In Figma: ${plain(figma)}`,
		notes.length === 0 ? "" : `Rules: ${notes.map((note) => blob + note).join(", ")}`,
		`Code: ${component} in @gitbutler/but-ui-react/${file}`,
	];
	const id = `${node[1]}:${node[2]}`;
	const taken = id in descriptions ? descriptions[id] : undefined;
	if (taken !== undefined) {
		problems.push(
			`${storiesFile}: links ${taken.component}'s drawing, ${id}; a drawing describes one component`,
		);
	}
	if (!MOVED.has(component)) {
		descriptions[id] ??= { component, file, description: "" };
		continue;
	}
	descriptions[id] = {
		component,
		file,
		description: lines.filter((line) => line !== "").join("\n\n"),
	};
}

const moved = Object.entries(descriptions).filter(([, { component }]) => MOVED.has(component));
const json = `${JSON.stringify(
	{
		$description:
			"Generated by scripts/figma-descriptions.mjs from each component's JSDoc; edit the JSDoc, not this file. Each 🍑 But UI node's description, by node id.",
		...Object.fromEntries(moved),
	},
	null,
	"\t",
)}\n`;

if (!process.argv.includes("--check")) {
	if (problems.length > 0) {
		process.stderr.write(
			`figma-descriptions:\n${problems.map((line) => `  ${line}`).join("\n")}\n`,
		);
		process.exit(1);
	}
	writeFileSync(out, json);
	process.stdout.write(`figma-descriptions: ${String(moved.length)} components written.\n`);
} else {
	if (!existsSync(out) || readFileSync(out, "utf8") !== json) {
		problems.push(
			"figma-descriptions.json is out of date: run node scripts/figma-descriptions.mjs",
		);
	}
	const figmaKey = process.env["FIGMA_API_KEY"] ?? "";
	const figmaFile = process.env["FIGMA_FILE_KEY"] ?? "";
	const askFigma = figmaKey !== "" && figmaFile !== "";
	if (askFigma) {
		const response = await fetch(
			`https://api.figma.com/v1/files/${figmaFile}/nodes?ids=${moved.map(([id]) => id).join(",")}&depth=1`,
			{ headers: { "X-Figma-Token": figmaKey } },
		);
		if (!response.ok) throw new Error(`Figma answered ${String(response.status)}`);
		const { nodes } =
			/** @type {{ nodes: Record<string, { document?: { description?: string } } | null> }} */ (
				await response.json()
			);
		for (const [id, { component, description }] of moved) {
			const drawn = nodes[id]?.document?.description;
			if (drawn === undefined) problems.push(`${component} (${id}): no such node in 🍑 But UI`);
			else if (unescaped(drawn) !== description)
				problems.push(`${component} (${id}): Figma's description differs from the JSDoc's`);
		}
	}
	if (problems.length > 0) {
		process.stderr.write(
			`figma-descriptions:\n${problems.map((line) => `  ${line}`).join("\n")}\n`,
		);
		process.exit(1);
	}
	process.stdout.write(
		`figma-descriptions: ${String(moved.length)} components up to date${askFigma ? ", and Figma matches" : "; Figma not compared without FIGMA_API_KEY"}.\n`,
	);
}
