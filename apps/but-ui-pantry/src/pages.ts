import {
	isStory,
	storyId,
	storyName,
	type Story,
} from "@gitbutler/but-ui-react/stories/stories.ts";
import { pageList } from "./page-list.ts";

/**
 * A page: a design note, the Adoption page, or a component's stories with the notes its JSDoc
 * names (`notes`), the component and the library file it comes from (`module`), how many stories it has, its frame in
 * Figma, and whether the GPUI client draws that component too (`gpui`, from `crates/but-ui-gpui`).
 */
export type Page = {
	id: string;
	title: string;
	file: string;
	notes?: Array<string>;
	component?: string;
	module?: string;
	stories?: number;
	figma?: string;
	gpui?: boolean;
};
export const pages: Array<Page> = pageList;

export const isDesignPage = (page: Page) => page.id.startsWith("design-");

/** The Colours page draws the stylesheet itself, not a note. */
const isColoursPage = (page: Page) => page.file.endsWith("/tokens.css");

export const isAdoptionPage = (page: Page) => page.id === "components-adoption";

/** The pages under their headings: a design page's folder, a component's group. */
export function groupPages(list: Array<Page>): Array<[heading: string, pages: Array<Page>]> {
	const groups = new Map<string, Array<Page>>();
	for (const page of list) {
		const heading = page.title.split("/").slice(-2, -1)[0] ?? page.title;
		groups.set(heading, [...(groups.get(heading) ?? []), page]);
	}
	return [...groups];
}

export const pageName = (page: Page) => page.title.split("/").at(-1) ?? page.title;

export const findPage = (id: string | null) => pages.find((page) => page.id === id);

/**
 * The page a design note opens: its own, or for a component's note, the first component page that
 * shows it. `note` is its path from this package's folder, `../../packages/but-ui/design/components/buttons.md`.
 */
export const notePage = (note: string) =>
	pages.find((page) => page.file === note || page.notes?.includes(note));

const storyModules = import.meta.glob<Record<string, unknown>>(
	"../../../packages/but-ui-react/src/*.stories.tsx",
);
const storySources = import.meta.glob<string>("../../../packages/but-ui-react/src/*.stories.tsx", {
	query: "?raw",
	import: "default",
});
const componentSources = import.meta.glob<string>(
	["../../../packages/but-ui-react/src/*.tsx", "!../../../packages/but-ui-react/src/*.stories.tsx"],
	{
		query: "?raw",
		import: "default",
	},
);
const markdown = import.meta.glob<string>(
	[
		"../../../packages/but-ui-react/design/*/*.md",
		"../../../packages/but-ui/DESIGN.md",
		"../../../packages/but-ui/design/*/*.md",
	],
	{
		query: "?raw",
		import: "default",
	},
);

const stylesheet = import.meta.glob<string>("../../../packages/but-ui/tokens/tokens.css", {
	query: "?raw",
	import: "default",
});

export type StoryEntry = {
	id: string;
	name: string;
	description: string | undefined;
	story: Story;
};

export type ComponentPageData = {
	kind: "component";
	description: string | undefined;
	/** The Markdown of each design note the component's `@see` tags name. */
	notes: Array<string>;
	stories: Array<StoryEntry>;
};
export type DesignPageData = { kind: "design"; source: string };
export type ColoursPageData = { kind: "colours"; source: string };
export type AdoptionPageData = { kind: "adoption"; snapshot: Adoption };

/** What but.dev uses of the library, as `scripts/adoption.mjs` read it off a but-dev commit. */
export type Adoption = {
	commit: string;
	date: string;
	/** The commit of this repository but.dev pins the library at. */
	pin: string | null;
	/** How many of the library's commits on master that pin doesn't have. */
	behind: number | null;
	/** How many of but.dev's files import from the library. */
	files: number;
	/** By the library's file: how many but.dev files import from it, and each name they import. */
	modules: Record<
		string,
		{ files: number; names: Record<string, { files: number; uses: number }> }
	>;
};

/** A JSDoc block's text, less its stars and its tags (`@import …`). */
function jsdoc(comment: string | undefined): string | undefined {
	const text = comment
		?.split("\n")
		.map((line) => line.replace(/^\s*\/?\*+\/?\s?/, "").replace(/\s*\*\/$/, ""))
		.join("\n")
		.split(/(?:^|\n)@\w/)[0]
		?.trim();
	return text === "" ? undefined : text;
}

/** The JSDoc right before `export … Name`. */
const docOf = (source: string, name: string) =>
	jsdoc(
		new RegExp(
			String.raw`(\/\*\*(?:(?!\*\/)[\s\S])*\*\/)\s*export (?:const|function|class) ${name}\b`,
		).exec(source)?.[1],
	);

async function loadComponentPage(page: Page): Promise<ComponentPageData> {
	const path = `../../../packages/but-ui-react/src/${page.file}`;
	const [module, source, notes] = await Promise.all([
		storyModules[path]?.(),
		storySources[path]?.(),
		Promise.all((page.notes ?? []).map(async (note) => markdown[`../${note}`]?.())),
	]);
	if (module === undefined || source === undefined) throw new Error(`No stories at ${path}`);

	// The component's JSDoc, from the file the stories import it from.
	const component = /\bstories(?:<[^>]*>)?\(\{[\s\S]*?\bcomponent: (\w+)/.exec(source)?.[1];
	const from =
		component === undefined
			? undefined
			: new RegExp(String.raw`import \{[^}]*\b${component}\b[^}]*\} from "\./(\w+\.tsx)"`).exec(
					source,
				)?.[1];
	const componentSource =
		from === undefined
			? undefined
			: await componentSources[`../../../packages/but-ui-react/src/${from}`]?.();

	const entries = Object.entries(module).flatMap(([name, value]) =>
		isStory(value) ? [{ name, story: value }] : [],
	);
	return {
		kind: "component",
		description:
			component === undefined || componentSource === undefined
				? undefined
				: docOf(componentSource, component),
		notes: notes.filter((note) => note !== undefined),
		// In the file's order, so `Default` leads.
		stories: entries
			.sort((a, b) => a.story.order - b.story.order)
			.map(({ name, story }) => ({
				id: storyId(story.meta, name),
				name: storyName(name),
				description: docOf(source, name),
				story,
			})),
	};
}

async function loadDesignPage(page: Page): Promise<DesignPageData> {
	const source = await markdown[`../${page.file}`]?.();
	if (source === undefined) throw new Error(`No Markdown at ${page.file}`);
	return { kind: "design", source };
}

async function loadColoursPage(page: Page): Promise<ColoursPageData> {
	const source = await stylesheet[`../${page.file}`]?.();
	if (source === undefined) throw new Error(`No stylesheet at ${page.file}`);
	return { kind: "colours", source };
}

async function loadAdoptionPage(): Promise<AdoptionPageData> {
	const snapshot = (await import("./adoption.json")).default as Adoption;
	return { kind: "adoption", snapshot };
}

type PageData = ComponentPageData | DesignPageData | ColoursPageData | AdoptionPageData;

const loads = new Map<string, Promise<PageData>>();

/**
 * A page's content, loaded once. The promise carries its result once settled, the way React's
 * `use` reads it, so a page already loaded renders without suspending.
 */
export function loadPage(page: Page): Promise<PageData> {
	let load = loads.get(page.id);
	if (load === undefined) {
		const started: Promise<PageData> = isAdoptionPage(page)
			? loadAdoptionPage()
			: isColoursPage(page)
				? loadColoursPage(page)
				: isDesignPage(page)
					? loadDesignPage(page)
					: loadComponentPage(page);
		void started.then(
			(value) => {
				const settled: Promise<PageData> & { status?: "fulfilled"; value?: PageData } = started;
				settled.status = "fulfilled";
				settled.value = value;
			},
			() => undefined,
		);
		loads.set(page.id, started);
		load = started;
	}
	return load;
}
