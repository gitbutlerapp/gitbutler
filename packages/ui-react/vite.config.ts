import react from "@vitejs/plugin-react";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";

/** The order of the sidebar's headings: the design notes, then ⚛️ Core's component pages. */
const groups = [
	"Design",
	"Getting started",
	"Foundations",
	"Content",
	"Components",
	"Patterns",
	"Actions",
	"Inputs",
	"Overlays",
	"Content & status",
	"Preview",
	"Sidebar",
	"Icons",
];

const designFolders = ["getting-started", "foundations", "content", "components", "patterns"];

const capitalise = (words: string) =>
	words.charAt(0).toUpperCase() + words.slice(1).replaceAll("-", " ");

/**
 * Fills in `docs/page-list.ts`, every page the site lists: a design note per Markdown file, a
 * component per stories file. Read off the files here, so the sidebar knows the pages without
 * loading every component.
 */
const docsPages = (): Plugin => ({
	name: "docs-pages",
	load(id) {
		if (!id.endsWith("/docs/page-list.ts")) return undefined;
		const read = (file: string) => {
			this.addWatchFile(resolve(file));
			return readFileSync(file, "utf8");
		};
		const heading = (source: string) => /^# (.+)$/m.exec(source)?.[1] ?? "";

		const design = [
			{ id: "design-overview", title: "Design/Overview", file: "DESIGN.md" },
			...designFolders.flatMap((folder) =>
				readdirSync(`design/${folder}`)
					.filter((file) => file.endsWith(".md"))
					.map((file) => ({
						id: `design-${folder}-${file.replace(/\.md$/, "")}`,
						title: `Design/${capitalise(folder)}/${heading(read(`design/${folder}/${file}`))}`,
						file: `design/${folder}/${file}`,
					})),
			),
		];
		const components = readdirSync("src")
			.filter((file) => file.endsWith(".stories.tsx"))
			.map((file) => {
				// The page's title and id are the `stories({…})` call's, not a story's data.
				const source = read(`src/${file}`).split(/\bstories(?:<[^>]*>)?\(/)[1] ?? "";
				return {
					id: /\bid: "([^"]+)"/.exec(source)?.[1] ?? file,
					title: /\btitle: "([^"]+)"/.exec(source)?.[1] ?? file,
					file,
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

/** The docs site, `pnpm demos`: every component's page, drawn with the components. */
export default defineConfig({
	root: "docs",
	plugins: [react(), docsPages()],
	server: { port: 6007 },
	build: { outDir: "../dist", emptyOutDir: true },
});
