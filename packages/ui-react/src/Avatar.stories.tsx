import preview from "#storybook/preview";
import { Avatar, type AvatarSize } from "./Avatar.tsx";

const sizes: Array<AvatarSize> = [14, 16, 18];

const meta = preview.meta({
	component: Avatar,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2159-1542",
		},
	},
	args: { src: "https://avatars.githubusercontent.com/u/7453394?v=4" },
	argTypes: { size: { control: "select", options: sizes } },
});

export const Image = meta.story({});

/** No picture: the circle keeps the row's rhythm. */
export const Placeholder = meta.story({ args: { src: null } });

export const Sizes = meta.story({
	render: (args) => (
		<div style={{ display: "flex", gap: 12, alignItems: "center" }}>
			{sizes.map((size) => (
				<Avatar key={size} {...args} size={size} />
			))}
			{sizes.map((size) => (
				<Avatar key={`p${size}`} src={null} size={size} />
			))}
		</div>
	),
});
