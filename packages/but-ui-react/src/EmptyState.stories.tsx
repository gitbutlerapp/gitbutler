import type { FC } from "react";
import { stories } from "../stories/stories.ts";
import { Button } from "./Button.tsx";
import { EmptyState } from "./EmptyState.tsx";
import { Icon } from "./Icon.tsx";

const meta = stories({
	title: "Content & status/EmptyState",
	id: "components-emptystate",
	component: EmptyState,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=3454-84769",
		},
	},
	args: {
		illustration: "cactus",
		title: "Your workspace is empty",
		description: "You have 5 branches to pick from",
	},
});

/** A plain box with a height, as a pane gives it: the block centres itself in it, a little above
 * the middle. */
const pane = (Story: FC) => (
	<div
		style={{
			width: 360,
			height: 520,
			backgroundColor: "var(--but-muted)",
			borderRadius: 6,
		}}
	>
		<Story />
	</div>
);

/** A card, as a sidebar drawer or a popup's list holds it: `tidy` is as tall as its content, and
 * the card's padding is the room around it. */
const card = (Story: FC) => (
	<div
		style={{
			width: 360,
			padding: "28px 16px",
			border: "1px solid var(--but-border-subtle)",
			borderRadius: "var(--but-radius-card)",
			backgroundColor: "var(--but-background)",
		}}
	>
		<Story />
	</div>
);

export const Default = meta.story({
	decorators: [pane],
	args: {
		title: "Your workspace is empty",
		description: "You have 5 branches to pick from",
		children: (
			<>
				<Button variant="gray">
					<Icon name="list" />
					See all
				</Button>
				<Button variant="outline">
					<Icon name="plus" />
					New branch
				</Button>
			</>
		),
	},
});

/** One action stays quiet: there is no second button for it to rank above. */
export const OneAction = meta.story({
	decorators: [pane],
	args: {
		title: "No branches yet",
		description: "Your first commit will start one",
		children: (
			<Button variant="outline">
				<Icon name="plus" />
				New branch
			</Button>
		),
	},
});

/** Nothing to do about it, so nothing to press. */
export const NoActions = meta.story({
	decorators: [pane],
	args: {
		title: "Nothing to review",
		description: "Pull requests you are asked to look at will show up here",
	},
});

/** Dropped where a panel is too short to hold one. */
export const WithoutIllustration = meta.story({
	decorators: [pane],
	args: {
		title: "Your workspace is empty",
		description: "You have 5 branches to pick from",
		illustration: undefined,
		children: (
			<Button variant="outline">
				<Icon name="plus" />
				New branch
			</Button>
		),
	},
});

/** A popup's search that found nothing: the line and a way back to the full list, as tall as its
 * content in the card that holds it. */
export const Tidy = meta.story({
	decorators: [card],
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=3454-84770",
		},
	},
	args: {
		variant: "tidy",
		illustration: undefined,
		title: undefined,
		description: "No matching files found",
		children: (
			<Button variant="outline" size="small">
				Clear search
			</Button>
		),
	},
});
