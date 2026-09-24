import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { Tooltip as BaseTooltip } from "@base-ui/react";
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
				<BaseTooltip.Provider>
					<Story />
				</BaseTooltip.Provider>
			</div>
		),
	],
});

export const Playground = meta.story({
	args: {
		content: "This is a tooltip",
		kbd: "Mod+A",
		side: "top",
		children: <Button>Hover me</Button>,
	},
});

export const IconOnly = meta.story({
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
