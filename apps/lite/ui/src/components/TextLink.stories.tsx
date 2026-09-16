import preview from "#storybook/preview";
import { TextLink } from "./TextLink.tsx";

const meta = preview.meta({
	component: TextLink,
	args: {
		href: "https://github.com/gitbutlerapp/gitbutler/pull/15941",
		children: "#15941",
		// Storybook has no system browser to hand the link to.
		onClick: (evt) => evt.preventDefault(),
	},
});

export const Default = meta.story({});

export const InText = meta.story({
	render: (args) => (
		<p className="text-12" style={{ color: "var(--text-2)" }}>
			We’d love to hear what you think. <TextLink {...args}>Share feedback on Discord</TextLink>
		</p>
	),
	args: {
		href: "https://discord.gg/MmFkmaJ42D",
	},
});
