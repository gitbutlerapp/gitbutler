import react from "@vitejs/plugin-react";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";

/** The order of the sidebar's headings: the design notes, the Adoption page, then 🍑 But UI's
 * component pages. */
const groups = [
	"Design",
	"Getting started",
	"Foundations",
	"Content",
	"Patterns",
	"Components",
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

/** The notes a JSDoc block names in its `@see` tags, as paths from this package: a component's own
 * note, or a pattern the component takes part in, which keeps its own page too. */
const componentNotes = (jsdoc: string) =>
	[
		...jsdoc.matchAll(/@see packages\/but-ui\/(design\/(?:components|patterns)\/[a-z-]+\.md)/g),
	].flatMap(([, note]) => (note === undefined ? [] : [`../../packages/but-ui/${note}`]));

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

/** 🍑 But UI, where every component's drawing is. */
const FIGMA_FILE = "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI";

/**
 * The first node a stories file links, as `scripts/figma-descriptions.mjs` finds it: in a link,
 * `node-id=217-317`, or handed to a helper that makes one, `figma("217-317")`.
 */
const FIGMA_NODE = /(?:node-id=|\w\(")(\d+-\d+)/;

const capitalise = (words: string) =>
	words.charAt(0).toUpperCase() + words.slice(1).replaceAll("-", " ");

/**
 * Fills in `src/page-list.ts`, every page the site lists: a design note per Markdown file, the
 * Colours page, the Adoption page, and a component per stories file with the notes its component's
 * `@see` tags name, the file it comes from, its stories and its frame in Figma.
 * Read off the files here, so the sidebar knows the pages without loading every component.
 */
const docsPages = (): Plugin => ({
	name: "docs-pages",
	// The list is read once, so a note or stories file added or removed while the server runs needs
	// the list read again: the folders it comes from are watched, and a file coming or going there
	// throws the list away and reloads the site.
	configureServer(server) {
		const folders = [
			...designFolders.map(([dir, folder]) => `${dir}/${folder}`),
			"../../packages/but-ui/design/components",
			"../../packages/but-ui-react/src",
			gpuiSource,
		].map((folder) => resolve(folder));
		server.watcher.add(folders);
		const relist = (file: string) => {
			if (!folders.some((folder) => file.startsWith(`${folder}/`))) return;
			for (const page of server.moduleGraph.getModulesByFile(resolve("src/page-list.ts")) ?? [])
				server.moduleGraph.invalidateModule(page);
			server.ws.send({ type: "full-reload" });
		};
		server.watcher.on("add", relist);
		server.watcher.on("unlink", relist);
	},
	load(id) {
		if (!id.endsWith("/but-ui-pantry/src/page-list.ts")) return undefined;
		const read = (file: string) => {
			this.addWatchFile(resolve(file));
			return readFileSync(file, "utf8");
		};
		const heading = (source: string) => /^# (.+)$/m.exec(source)?.[1] ?? "";

		const design = [
			{ id: "design-overview", title: "Design/Overview", file: "../../packages/but-ui/DESIGN.md" },
			// The colour tokens, drawn from the stylesheet that defines them rather than from a note.
			{
				id: "design-foundations-colours",
				title: "Design/Foundations/Colours",
				file: "../../packages/but-ui/tokens/tokens.css",
			},
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
				const node = FIGMA_NODE.exec(stories)?.[1];
				const figma = node === undefined ? undefined : `${FIGMA_FILE}?node-id=${node}`;
				// Stories with no `component` draw the file named for their title: `Icon.tsx`.
				const name = component ?? title.split("/").at(-1) ?? "";
				const module =
					from ??
					(existsSync(`../../packages/but-ui-react/src/${name}.tsx`) ? `${name}.tsx` : undefined);
				return {
					id: /\bid: "([^"]+)"/.exec(source)?.[1] ?? file,
					title,
					file,
					notes: componentNotes(jsdoc),
					component: name,
					...(module === undefined ? {} : { module }),
					stories: stories.match(/\bmeta\.story\(/g)?.length ?? 0,
					...(figma === undefined ? {} : { figma }),
					...(gpui.has(title.split("/").at(-1) ?? "") ? { gpui: true } : {}),
				};
			});
		const group = (title: string) => groups.indexOf(title.split("/").slice(-2, -1)[0] ?? "");
		// What but.dev uses of every component, which the Components card opens.
		const adoption = {
			id: "components-adoption",
			title: "Components/Adoption",
			file: "adoption.json",
		};
		const pages = [...design, adoption, ...components].sort((a, b) =>
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
