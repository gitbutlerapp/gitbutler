import { play } from "./play.ts";
import { isStory, storyName } from "./stories.ts";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, test } from "vitest";
import "./site.css";

const modules = import.meta.glob<Record<string, unknown>>("../src/*.stories.tsx", { eager: true });

let cleanup: (() => void) | undefined;
afterEach(() => {
	cleanup?.();
	cleanup = undefined;
});

for (const [path, module] of Object.entries(modules)) {
	const stories = Object.entries(module).flatMap(([name, value]) =>
		isStory(value) ? [{ name, story: value }] : [],
	);
	describe(stories[0]?.story.meta.title ?? path, () => {
		for (const { name, story } of stories) {
			test(storyName(name), async () => {
				const element = document.createElement("div");
				document.body.append(element);
				const errors: Array<unknown> = [];
				const root = createRoot(element, {
					onUncaughtError: (error) => errors.push(error),
					onRecoverableError: (error) => errors.push(error),
				});
				cleanup = () => {
					root.unmount();
					element.remove();
				};
				flushSync(() => root.render(<story.Component />));
				// A frame for effects, as the story would get on screen.
				await new Promise(requestAnimationFrame);
				expect(errors).toEqual([]);
				await play(story, element);
				expect(errors).toEqual([]);
			});
		}
	});
}
