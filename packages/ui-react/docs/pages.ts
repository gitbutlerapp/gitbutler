import { isStory, storyId, storyName, type Story } from "./stories.ts";
import { pageList } from "./page-list.ts";

export type Page = { id: string; title: string; file: string };
export const pages: Array<Page> = pageList;

export const isDesignPage = (page: Page) => page.id.startsWith("design-");

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

const storyModules = import.meta.glob<Record<string, unknown>>("../src/*.stories.tsx");
const storySources = import.meta.glob<string>("../src/*.stories.tsx", {
	query: "?raw",
	import: "default",
});
const componentSources = import.meta.glob<string>(["../src/*.tsx", "!../src/*.stories.tsx"], {
	query: "?raw",
	import: "default",
});
const markdown = import.meta.glob<string>(["../DESIGN.md", "../design/*/*.md"], {
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
	stories: Array<StoryEntry>;
};
export type DesignPageData = { kind: "design"; source: string };

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
	const path = `../src/${page.file}`;
	const [module, source] = await Promise.all([storyModules[path]?.(), storySources[path]?.()]);
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
		from === undefined ? undefined : await componentSources[`../src/${from}`]?.();

	const entries = Object.entries(module).flatMap(([name, value]) =>
		isStory(value) ? [{ name, story: value }] : [],
	);
	return {
		kind: "component",
		description:
			component === undefined || componentSource === undefined
				? undefined
				: docOf(componentSource, component),
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

type PageData = ComponentPageData | DesignPageData;

const loads = new Map<string, Promise<PageData>>();

/**
 * A page's content, loaded once. The promise carries its result once settled, the way React's
 * `use` reads it, so a page already loaded renders without suspending.
 */
export function loadPage(page: Page): Promise<PageData> {
	let load = loads.get(page.id);
	if (load === undefined) {
		const started: Promise<PageData> = isDesignPage(page)
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
