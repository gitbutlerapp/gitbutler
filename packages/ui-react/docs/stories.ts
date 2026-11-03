import type { BoundFunctions, queries } from "@testing-library/dom";
import type { UserEvent } from "@testing-library/user-event";
import type { ComponentType, FC, ReactNode } from "react";
import { createElement } from "react";

/** A story's link to its drawing in ⚛️ Core. */
export type Design = { type: string; url: string };

export type Parameters = {
	design?: Design;
	/** `fullscreen` fills the frame with no padding, `centered` centres the story in it. */
	layout?: "fullscreen" | "centered";
};

/** Wraps a story, as `(Story) => <div style={…}><Story /></div>`. */
export type Decorator = (Story: FC) => ReactNode;

/**
 * Steps run once the story is on screen: in the story tests, and in the story's own frame. Its
 * `canvas` queries the story's root, as Testing Library's `within` does.
 */
export type Play = (context: {
	canvas: BoundFunctions<typeof queries>;
	canvasElement: HTMLElement;
	userEvent: UserEvent;
}) => Promise<void>;

type Annotations<Props> = {
	args?: Partial<Props>;
	parameters?: Parameters;
	decorators?: Array<Decorator>;
	play?: Play;
	render?: (args: Props) => ReactNode;
};

export type Meta<Props> = Annotations<Props> & {
	/** The sidebar heading and the page's name: `"Overlays/Modal"`. */
	title: string;
	/** The page's address, which stays when the title moves group: `"components-modal"`. */
	id: string;
	component?: ComponentType<Props>;
};

export type Story = {
	meta: Meta<never>;
	parameters: Parameters;
	play: Play | undefined;
	/** The story with its args and decorators, ready to render. */
	Component: FC;
	/** Where the story comes in its file, so the page keeps the file's order. */
	order: number;
};

let made = 0;

/**
 * A component's stories: the page's title and address, and what every story on it shares.
 *
 * ```tsx
 * const meta = stories({ title: "Content & status/Badge", id: "components-badge", component: Badge });
 * export const Default = meta.story({ args: { children: "42" } });
 * ```
 */
export function stories<Props extends object>(meta: Meta<Props>) {
	return {
		/** A story; a `render` may take more args than the component, as a toggle for its icon. */
		story<StoryProps extends Props = Props>(story: Annotations<StoryProps> = {}): Story {
			const args = { ...meta.args, ...story.args } as StoryProps;
			const render = (): ReactNode => {
				if (story.render) return story.render(args);
				if (meta.render) return meta.render(args);
				return meta.component ? createElement(meta.component, args) : null;
			};
			// Story decorators sit inside the page's, as the closer wrapper.
			const decorators = [...(story.decorators ?? []), ...(meta.decorators ?? [])];
			const Component = decorators.reduce<FC>((Inner, decorate) => () => decorate(Inner), render);
			return {
				meta: meta as Meta<never>,
				parameters: { ...meta.parameters, ...story.parameters },
				play: story.play ?? meta.play,
				Component,
				order: made++,
			};
		},
	};
}

export const isStory = (value: unknown): value is Story =>
	typeof value === "object" && value !== null && "Component" in value && "order" in value;

/** `WithIcon` → `With Icon`, `CIStates` → `CI States`. */
export const storyName = (exportName: string) =>
	exportName.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/([A-Z])([A-Z][a-z])/g, "$1 $2");

/** `WithIcon` → `with-icon`, the second half of a story's address. */
export const storyId = (meta: { id: string }, exportName: string) =>
	`${meta.id}--${storyName(exportName).toLowerCase().replaceAll(" ", "-")}`;
