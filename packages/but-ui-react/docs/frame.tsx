import { findPage, loadPage } from "./pages.ts";
import { play } from "./play.ts";
import { paintAccent, paintTheme, type Accent } from "./site.ts";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";

const accentOf = (value: string | null | undefined): Accent =>
	value === "blue" || value === "purple" ? value : "teal";

/**
 * One story alone, `?story=components-modal--confirm`: the frame an overlay's example opens in,
 * and the page to screenshot a story on. Framed on a site page it follows the page's theme and
 * accent; opened on its own, `&theme=dark` and `&accent=blue` pick them.
 */
export async function renderFrame(id: string) {
	const search = new URLSearchParams(window.location.search);
	const site = sitePage();
	const followSite = () => {
		paintTheme(
			site?.classList.contains("dark") === true
				? "dark"
				: site?.classList.contains("light") === true
					? "light"
					: "system",
		);
		paintAccent(accentOf(site?.dataset["butAccent"]));
	};
	if (site === undefined) {
		paintTheme(search.get("theme") === "dark" ? "dark" : "light");
		paintAccent(accentOf(search.get("accent")));
	} else {
		followSite();
		new MutationObserver(followSite).observe(site, {
			attributeFilter: ["class", "data-but-accent"],
		});
	}

	const page = findPage(id.split("--")[0] ?? "");
	const data = page && (await loadPage(page));
	const entry = data?.kind === "component" ? data.stories.find((s) => s.id === id) : undefined;
	const root = document.getElementById("root");
	if (entry === undefined || root === null) throw new Error(`No story ${id}`);

	document.body.dataset["layout"] = entry.story.parameters.layout ?? "padded";
	if (entry.story.parameters.width !== undefined)
		root.style.width = `${entry.story.parameters.width}px`;
	flushSync(() => createRoot(root).render(<entry.story.Component />));
	await play(entry.story, root);
}

/** The site's root, when this frame is on one of its pages. */
function sitePage(): HTMLElement | undefined {
	try {
		return window.parent === window ? undefined : window.parent.document.documentElement;
	} catch {
		return undefined;
	}
}
