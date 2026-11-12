import { stories } from "../docs/stories.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { Tooltip as BaseTooltip } from "@base-ui/react";
import { Tooltip } from "./Tooltip.tsx";

const meta = stories({
	title: "Overlays/Tooltip",
	id: "components-tooltip",
	component: Tooltip,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=426-323&t=zrLHwmcDOZvnbONB-1",
		},
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "64px" }}>
				<BaseTooltip.Provider>
					<Story />
				</BaseTooltip.Provider>
			</div>
		),
	],
});

export const Default = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=1302-4798",
		},
	},
	args: {
		content: "This is a tooltip",
		kbd: "Mod+A",
		side: "bottom",
		children: <Button>Hover me</Button>,
	},
});

export const IconOnly = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=1302-4796",
		},
	},
	render: () => (
		<Tooltip content="New branch" kbd="Mod+B">
			<Button variant="ghost" iconOnly aria-label="New branch">
				<Icon name="plus" />
			</Button>
		</Tooltip>
	),
});

export const OnDisabledButton = meta.story({
	render: () => (
		<Tooltip content="Nothing to commit">
			<Button variant="pop" focusableWhenDisabled disabled>
				Commit
			</Button>
		</Tooltip>
	),
});
