import { findPage, loadPage, pages, type Page } from "./pages.ts";
import type { MouseEvent } from "react";

/** The page the address names, or the design overview. */
export const currentPage = (): Page | undefined =>
	findPage(new URLSearchParams(window.location.search).get("page")) ?? pages[0];

const listeners = new Set<() => void>();
export const subscribePage = (listener: () => void) => {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
};
const showPage = () => {
	for (const listener of listeners) listener();
};
window.addEventListener("popstate", showPage);

export const pageHref = (id: string) => `?page=${id}`;

/**
 * A link to a page. A plain click loads the page first and then swaps it in, so the one on
 * screen stays until the next is ready; a click that opens a new tab goes where the address says.
 */
export const pageLink = (id: string) => ({
	href: pageHref(id),
	onClick: (event: MouseEvent<HTMLAnchorElement>) => {
		const page = findPage(id);
		if (
			page === undefined ||
			event.button !== 0 ||
			event.metaKey ||
			event.ctrlKey ||
			event.shiftKey ||
			event.altKey
		)
			return;
		event.preventDefault();
		void loadPage(page).then(() => {
			window.history.pushState(null, "", pageHref(id));
			document.querySelector(".docs-site-main")?.scrollTo(0, 0);
			showPage();
		});
	},
});

export type Theme = "system" | "light" | "dark";

/**
 * The theme is the root's `light` or `dark` class, which the tokens follow; without either they
 * follow the system's.
 */
export const paintTheme = (theme: Theme, root = document.documentElement) => {
	root.classList.toggle("dark", theme === "dark");
	root.classList.toggle("light", theme === "light");
};

const themeKey = "ui-react-docs-theme";

/** The theme last picked here, else the system's. */
export function savedTheme(): Theme {
	try {
		const saved = window.localStorage.getItem(themeKey);
		if (saved === "light" || saved === "dark") return saved;
	} catch {
		// Storage can be off; the system's theme stands in.
	}
	return "system";
}

export function pickTheme(theme: Theme) {
	paintTheme(theme);
	try {
		window.localStorage.setItem(themeKey, theme);
	} catch {
		// Kept for this visit only.
	}
}
