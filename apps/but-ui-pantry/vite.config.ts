import react from "@vitejs/plugin-react";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";

/** The order of the sidebar's headings: the design notes, then 🍑 But UI's component pages. */
const groups = [
	"Design",
	"Getting started",
	"Foundations",
	"Content",
	"Patterns",
	"Actions",
	"Inputs",
	"Overlays",
	"Content & status",
	"Files & diffs",
	"Illustrations",
	"Preview",
	"Sidebar",
	"Workspace tree",
	"Icons",
];

/** Where each heading's design notes are: the rules for every platform are but-ui's; only how to
 * build with this library is here. A component's own notes are no heading's: they show on the
 * component's page. */
const designFolders = [
	["../../packages/but-ui-react/design", "getting-started"],
	["../../packages/but-ui/design", "foundations"],
	["../../packages/but-ui/design", "content"],
	["../../packages/but-ui/design", "patterns"],
] as const;

/** The component notes a JSDoc block names in its `@see` tags, as paths from this package. */
const componentNotes = (jsdoc: string) =>
	[...jsdoc.matchAll(/@see packages\/but-ui\/(design\/components\/[a-z-]+\.md)/g)].flatMap(
		([, note]) => (note === undefined ? [] : [`../../packages/but-ui/${note}`]),
	);

/** Where the GPUI client's design system is, whose components the docs site marks GPUI. */
const gpuiSource = "../../crates/but-ui-gpui/src";

/**
 * The components the GPUI client draws too: each module of `but-ui-gpui` names its
 * counterparts in its first doc paragraph ("as but-ui-react's `Button` and `DropdownButton`").
 */
const gpuiComponents = (read: (file: string) => string) =>
	new Set(
		readdirSync(gpuiSource)
			.filter((file) => file.endsWith(".rs"))
			.flatMap((file) => {
				const docs = (/^(?:\/\/!.*\n)+/.exec(read(`${gpuiSource}/${file}`))?.[0] ?? "")
					.replaceAll("//!", "")
					.split(/\n\s*\n/);
				const doc = (docs[0] ?? "").replace(/\s+/g, " ");
				const named = /but-ui-react's ([^.]*)/.exec(doc)?.[1] ?? "";
				return [...named.matchAll(/`(\w+)`/g)].flatMap(([, name]) => name ?? []);
			}),
	);

const capitalise = (words: string) =>
	words.charAt(0).toUpperCase() + words.slice(1).replaceAll("-", " ");

/**
 * Fills in `src/page-list.ts`, every page the site lists: a design note per Markdown file, a
 * component per stories file with the notes its component's `@see` tags name. Read off the files
 * here, so the sidebar knows the pages without loading every component.
 */
const docsPages = (): Plugin => ({
	name: "docs-pages",
	load(id) {
		if (!id.endsWith("/but-ui-pantry/src/page-list.ts")) return undefined;
		const read = (file: string) => {
			this.addWatchFile(resolve(file));
			return readFileSync(file, "utf8");
		};
		const heading = (source: string) => /^# (.+)$/m.exec(source)?.[1] ?? "";

		const design = [
			{ id: "design-overview", title: "Design/Overview", file: "../../packages/but-ui/DESIGN.md" },
			...designFolders.flatMap(([dir, folder]) =>
				readdirSync(`${dir}/${folder}`)
					.filter((file) => file.endsWith(".md"))
					.map((file) => ({
						id: `design-${folder}-${file.replace(/\.md$/, "")}`,
						title: `Design/${capitalise(folder)}/${heading(read(`${dir}/${folder}/${file}`))}`,
						file: `${dir}/${folder}/${file}`,
					})),
			),
		];
		const gpui = gpuiComponents(read);
		const components = readdirSync("../../packages/but-ui-react/src")
			.filter((file) => file.endsWith(".stories.tsx"))
			.map((file) => {
				// The page's title and id are the `stories({…})` call's, not a story's data.
				const stories = read(`../../packages/but-ui-react/src/${file}`);
				const source = stories.split(/\bstories(?:<[^>]*>)?\(/)[1] ?? "";
				// The component's JSDoc, from the file the stories import it from, as the page shows it.
				const component = /\bcomponent: (\w+)/.exec(source)?.[1];
				const from =
					component === undefined
						? undefined
						: new RegExp(
								String.raw`import \{[^}]*\b${component}\b[^}]*\} from "\./(\w+\.tsx)"`,
							).exec(stories)?.[1];
				const jsdoc =
					from === undefined
						? ""
						: (new RegExp(
								String.raw`(\/\*\*(?:(?!\*\/)[\s\S])*\*\/)\s*export (?:const|function|class) ${component}\b`,
							).exec(read(`../../packages/but-ui-react/src/${from}`))?.[1] ?? "");
				const title = /\btitle: "([^"]+)"/.exec(source)?.[1] ?? file;
				return {
					id: /\bid: "([^"]+)"/.exec(source)?.[1] ?? file,
					title,
					file,
					notes: componentNotes(jsdoc),
					...(gpui.has(title.split("/").at(-1) ?? "") ? { gpui: true } : {}),
				};
			});
		const group = (title: string) => groups.indexOf(title.split("/").slice(-2, -1)[0] ?? "");
		const pages = [...design, ...components].sort((a, b) =>
			group(a.title) === group(b.title)
				? a.title.localeCompare(b.title)
				: group(a.title) - group(b.title),
		);
		return `export const pageList = ${JSON.stringify(pages)};`;
	},
});

/** The pantry, `pnpm dev`: the design notes and every component's page, drawn with the components. */
export default defineConfig({
	root: "src",
	// The stories import React from but-ui-react's folder and the site from its own: one copy.
	resolve: { dedupe: ["react", "react-dom"] },
	plugins: [react(), docsPages()],
	server: { port: 6007 },
	build: { outDir: "../dist", emptyOutDir: true },
});
