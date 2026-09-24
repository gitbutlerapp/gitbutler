import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { EmptyState } from "./EmptyState.tsx";
import { Icon } from "./Icon.tsx";
import { illustrations, type IllustrationName } from "./illustrations.ts";

const meta = preview.meta({
	component: EmptyState,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=1852-4694",
		},
	},
	argTypes: {
		illustration: {
			control: "select",
			options: [undefined, ...(Object.keys(illustrations) as Array<IllustrationName>)],
		},
	},
	args: {
		illustration: "cactus",
		title: "Your workspace is empty",
		description: "You have 5 branches to pick from",
	},
	decorators: [
		// A plain box with a height, as a pane gives it: the block centres itself
		// in it, and the counterweight scales to it.
		(Story) => (
			<div
				style={{
					containerType: "size",
					width: 360,
					height: 520,
					backgroundColor: "var(--bg-2)",
					borderRadius: 6,
				}}
			>
				<Story />
			</div>
		),
	],
});

export const TwoActions = meta.story({
	args: {
		title: "Your workspace is empty",
		description: "You have 5 branches to pick from",
		children: (
			<>
				<Button variant="gray">
					See all
					<Icon name="list" />
				</Button>
				<Button variant="outline">
					New branch
					<Icon name="plus" />
				</Button>
			</>
		),
	},
});

/** One action stays quiet: there is no second button for it to rank above. */
export const OneAction = meta.story({
	args: {
		title: "No branches yet",
		description: "Your first commit will start one",
		children: (
			<Button variant="outline">
				New branch
				<Icon name="plus" />
			</Button>
		),
	},
});

/** Nothing to do about it, so nothing to press. */
export const NoActions = meta.story({
	args: {
		title: "Nothing to review",
		description: "Pull requests you are asked to look at will show up here",
	},
});

/** Dropped where a panel is too short to hold one. */
export const WithoutIllustration = meta.story({
	args: {
		title: "Your workspace is empty",
		description: "You have 5 branches to pick from",
		illustration: undefined,
		children: (
			<Button variant="outline">
				New branch
				<Icon name="plus" />
			</Button>
		),
	},
});
