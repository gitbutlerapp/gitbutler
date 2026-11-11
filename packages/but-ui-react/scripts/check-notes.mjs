#!/usr/bin/env node

/**
 * Checks that everything the design notes name still exists, so a rename can't leave a note
 * pointing at nothing. The notes are @gitbutler/but-ui's `DESIGN.md` and `design/`, and this
 * library's.
 *
 * What a note names in code (between backticks) is checked by its shape:
 *
 * - `--but-x`, anywhere in the span: a token tokens.css declares, or one of the few the library's
 *   base.css adds, as `--but-focus-ring`.
 * - Any other `--x` alone: a property the library's CSS or TSX sets or reads.
 * - `Name`: a component, or anything else the library exports.
 * - `file.tsx` and other file names: a file in the library; `packages/…` from the repository's
 *   root.
 * - `text-x`: a class in but-ui's text styles, or one of CSS's `text-*` values the notes quote.
 * - `camelCase`: a prop or another name the library's source uses.
 *
 * A link to another note must reach its file. Anything else in backticks (a variant's value, a
 * snippet of CSS, an example of copy) is prose and isn't checked.
 *
 * Figma nodes, as `1706-1682` after "node", are checked only with FIGMA_API_KEY and
 * FIGMA_FILE_KEY set, as when tokens are pulled; without them the check says it skipped them.
 *
 * Usage: node scripts/check-notes.mjs   (exit 1 on anything a note names that's gone)
 */

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(packageRoot, "../..");
const butUi = join(packageRoot, "../but-ui");
const src = join(packageRoot, "src");

/**
 * Every file under `dir`, recursively.
 * @param {string} dir
 * @returns {string[]}
 */
function files(dir) {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? files(path) : [path];
	});
}

/**
 * The first group of each match of `pattern` in `text`.
 * @param {string} text
 * @param {RegExp} pattern
 * @returns {string[]}
 */
function captures(text, pattern) {
	return [...text.matchAll(pattern)].map((match) => match[1]);
}

const notes = [
	join(butUi, "DESIGN.md"),
	...files(join(butUi, "design")),
	join(packageRoot, "DESIGN.md"),
	...files(join(packageRoot, "design")),
].filter((file) => file.endsWith(".md"));

const sources = files(src).filter((file) => /\.(tsx?|css)$/.test(file));
const sourceText = sources.map((file) => readFileSync(file, "utf8")).join("\n");
const tokens = new Set(
	[readFileSync(join(butUi, "tokens/tokens.css"), "utf8"), sourceText].flatMap((css) =>
		captures(css, /(--but-[\w-]+)\s*:/g),
	),
);
const textClasses = new Set(
	captures(readFileSync(join(butUi, "styles/text.css"), "utf8"), /\.(text-[\w-]+)/g),
);
/** CSS's own `text-wrap` values, which the line-breaks note quotes as `text-balance`. */
const cssTextValues = new Set(["text-balance", "text-pretty"]);
const exported = new Set(
	captures(sourceText, /export\s+(?:const|function|class|type|interface|enum)\s+(\w+)/g),
);
const fileNames = new Set(files(src).map((file) => file.split("/").at(-1) ?? ""));

/**
 * What's wrong with code span `span`, if it names something that's gone.
 * @param {string} span
 * @returns {string | undefined}
 */
function problem(span) {
	const missingTokens = captures(span, /(--but-[\w-]+)/g).filter((token) => !tokens.has(token));
	if (missingTokens.length > 0) return `${missingTokens.join(", ")}: no such token`;

	/** @type {[boolean, boolean, string][]} Each shape: whether `span` has it, whether it holds, and what's wrong if not. */
	const shapes = [
		[
			/^--[\w-]+$/.test(span) && !span.startsWith("--but-"),
			sourceText.includes(span),
			"the library never sets or reads it",
		],
		[
			/^[A-Z][A-Za-z]+$/.test(span),
			exported.has(span) || fileNames.has(`${span}.tsx`),
			"the library exports nothing by that name",
		],
		[span.startsWith("packages/"), existsSync(join(repoRoot, span)), "no such path"],
		[
			/^[\w.-]+\.(tsx|ts|css|mjs|md)$/.test(span),
			fileNames.has(span) || existsSync(join(packageRoot, span)),
			"no such file in the library",
		],
		[
			/^text-[\w-]+$/.test(span),
			textClasses.has(span) || cssTextValues.has(span),
			"no such text class",
		],
		[
			/^[a-z]+[A-Z]\w*$/.test(span),
			new RegExp(`\\b${span}\\b`).test(sourceText),
			"the library's source never uses it",
		],
	];
	const shape = shapes.find(([has]) => has);
	return shape === undefined || shape[1] ? undefined : `${span}: ${shape[2]}`;
}

/** @type {string[]} */
const problems = [];
/** @type {{ at: string, node: string }[]} */
const figmaNodes = [];
for (const file of notes) {
	const name = relative(repoRoot, file);
	const lines = readFileSync(file, "utf8").split("\n");
	let fenced = false;
	for (const [index, line] of lines.entries()) {
		if (line.startsWith("```")) fenced = !fenced;

		if (fenced) continue;

		const at = `${name}:${index + 1}`;
		for (const span of captures(line, /`([^`]+)`/g)) {
			const wrong = problem(span);
			if (wrong !== undefined) problems.push(`${at}: ${wrong}`);
		}
		for (const target of captures(line, /\]\(([^)#\s]+\.md)(?:#[^)]*)?\)/g)) {
			if (!/^https?:/.test(target) && !existsSync(join(dirname(file), target)))
				problems.push(`${at}: ${target}: the link reaches no note`);
		}
		for (const node of captures(line, /node `(\d+-\d+)`/g)) figmaNodes.push({ at, node });
	}
}

const figmaKey = process.env["FIGMA_API_KEY"] ?? "";
const figmaFile = process.env["FIGMA_FILE_KEY"] ?? "";
const askFigma = figmaNodes.length > 0 && figmaKey !== "" && figmaFile !== "";
if (askFigma) {
	const ids = figmaNodes.map(({ node }) => node.replace("-", ":"));
	const response = await fetch(
		`https://api.figma.com/v1/files/${figmaFile}/nodes?ids=${ids.join(",")}&depth=1`,
		{ headers: { "X-Figma-Token": figmaKey } },
	);
	if (!response.ok) throw new Error(`Figma answered ${String(response.status)}`);

	const { nodes } = /** @type {{ nodes: Record<string, unknown> }} */ (await response.json());
	for (const { at, node } of figmaNodes) {
		if (nodes[node.replace("-", ":")] == null)
			problems.push(`${at}: ${node}: no such node in 🍑 But UI`);
	}
}

if (problems.length > 0) {
	const one = problems.length === 1;
	process.stderr.write(
		`check-notes: ${String(problems.length)} name${one ? "" : "s"} in the design notes ${one ? "is" : "are"} gone:\n${problems.map((line) => `  ${line}`).join("\n")}\n`,
	);
	process.exit(1);
}
const figma =
	figmaNodes.length === 0
		? ""
		: askFigma
			? `, and ${String(figmaNodes.length)} Figma nodes`
			: `; ${String(figmaNodes.length)} Figma nodes skipped without FIGMA_API_KEY`;
process.stdout.write(
	`check-notes: ${String(notes.length)} notes, everything they name exists${figma}.\n`,
);
