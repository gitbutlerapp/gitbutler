import preview from "#storybook/preview";
import { Button } from "#ui/components/Button.tsx";

import { Tooltip } from "./Tooltip.tsx";

const meta = preview.meta({
	component: Tooltip,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=426-309&t=zrLHwmcDOZvnbONB-1",
		},
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "64px" }}>
				<Story />
			</div>
		),
	],
});

export const Playground = meta.story({
	argTypes: {
		positionerProps: {
			control: "select",
			options: ["top", "right", "bottom", "left"],
			mapping: {
				top: { side: "top" },
				right: { side: "right" },
				bottom: { side: "bottom" },
				left: { side: "left" },
			},
		},
	},
	args: {
		trigger: <Button>Hover me</Button>,
		content: "This is a tooltip",
		positionerProps: { side: "top" },
	},
});
