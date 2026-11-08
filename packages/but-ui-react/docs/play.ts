import type { Story } from "./stories.ts";
import { within } from "@testing-library/dom";
import { userEvent } from "@testing-library/user-event";

/** Runs a story's play function against the element it rendered into. */
export const play = async (story: Story, root: HTMLElement) =>
	story.play?.({ canvas: within(root), canvasElement: root, userEvent: userEvent.setup() });
