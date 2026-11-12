import { stories } from "../docs/stories.ts";
import { TextLink } from "./TextLink.tsx";

const meta = stories({
	title: "Actions/TextLink",
	id: "components-textlink",
	component: TextLink,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=2118-1509",
		},
	},
	args: {
		href: "https://github.com/gitbutlerapp/gitbutler/pull/15941",
		children: "#15941",
		// The docs site has no system browser to hand the link to.
		onClick: (evt) => evt.preventDefault(),
	},
});

export const Default = meta.story({});

export const InText = meta.story({
	render: (args) => (
		<p className="text-12" style={{ color: "var(--but-muted-foreground)" }}>
			We’d love to hear what you think. <TextLink {...args}>Share feedback on Discord</TextLink>
		</p>
	),
	args: {
		href: "https://discord.gg/MmFkmaJ42D",
	},
});
