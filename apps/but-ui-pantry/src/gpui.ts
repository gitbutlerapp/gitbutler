import { useSyncExternalStore } from "react";

/** Which client an example card shows: this library's story, or the GPUI client's shot of it. */
export type Renderer = "react" | "gpui";

// `scripts/gpui-shots.mjs` writes `<story id>.<appearance>.png`; only the stories it has shot
// offer the GPUI view.
const shots = import.meta.glob<string>("./gpui/*.png", {
	eager: true,
	query: "?url",
	import: "default",
});

/** The GPUI client's shots of a story, light and dark, at 2x, if it draws that story. */
export const gpuiShots = (storyId: string) => {
	const light = shots[`./gpui/${storyId}.light.png`];
	const dark = shots[`./gpui/${storyId}.dark.png`];
	return light === undefined || dark === undefined ? undefined : { light, dark };
};

/** The appearance the site is drawn in: the root's `dark`/`light` class, else the system's. */
export const useAppearance = (): "light" | "dark" =>
	useSyncExternalStore(
		(changed) => {
			const observer = new MutationObserver(changed);
			observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
			const media = window.matchMedia("(prefers-color-scheme: dark)");
			media.addEventListener("change", changed);
			return () => {
				observer.disconnect();
				media.removeEventListener("change", changed);
			};
		},
		() => {
			const root = document.documentElement.classList;
			if (root.contains("dark")) return "dark";
			if (root.contains("light")) return "light";
			return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
		},
	);
