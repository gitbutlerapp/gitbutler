import preview from "#storybook/preview";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Tooltip } from "@base-ui/react";
import { TooltipPopup } from "./Tooltip.tsx";

const meta = preview.meta({
	component: TooltipPopup,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=426-309&t=zrLHwmcDOZvnbONB-1",
		},
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "64px" }}>
				<Tooltip.Provider>
					<Story />
				</Tooltip.Provider>
			</div>
		),
	],
});

export const Playground = meta.story({
	args: {
		content: "This is a tooltip",
		kbd: "Mod+A",
	},
	render: (args) => (
		<Tooltip.Root>
			<Tooltip.Trigger className={getButtonClassName({})}>Hover me</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4} side="top">
					<Tooltip.Popup render={<TooltipPopup {...args} />} />
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	),
});
