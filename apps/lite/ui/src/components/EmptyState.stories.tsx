import preview from "#storybook/preview";
import { getButtonClassName } from "./Button.tsx";
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
		// The component is built to be centred by its host, and the counterweight
		// only reads as correct against a surface with room above and below it.
		(Story) => (
			<div
				style={{
					// As the app's host does: the counterweight scales to this box.
					containerType: "size",
					display: "flex",
					flexDirection: "column",
					justifyContent: "safe center",
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
				<button type="button" className={getButtonClassName({ variant: "gray" })}>
					See all
					<Icon name="list" />
				</button>
				<button type="button" className={getButtonClassName({ variant: "outline" })}>
					New branch
					<Icon name="plus" />
				</button>
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
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				New branch
				<Icon name="plus" />
			</button>
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
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				New branch
				<Icon name="plus" />
			</button>
		),
	},
});
