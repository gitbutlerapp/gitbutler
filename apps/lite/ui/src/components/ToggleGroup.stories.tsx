import preview from "#storybook/preview";
import { ToggleGroup, Toggle } from "@base-ui/react";
import { Icon } from "./Icon.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";

const meta = preview.meta({
	component: ToggleGroup,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=509-849",
		},
	},
});

export const Default = meta.story({
	render: () => (
		<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["list"]} aria-label="View mode">
			<Toggle render={<ToggleStyles />} value="list">
				List
			</Toggle>
			<Toggle render={<ToggleStyles />} value="tree">
				Tree
			</Toggle>
		</ToggleGroup>
	),
});

export const WithIcons = meta.story({
	render: () => (
		<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["list"]} aria-label="View mode">
			<Toggle render={<ToggleStyles />} value="list">
				<Icon name="list" size={14} />
				List
			</Toggle>
			<Toggle render={<ToggleStyles />} value="tree">
				<Icon name="folder-tree" size={14} />
				Tree
			</Toggle>
		</ToggleGroup>
	),
});

export const IconOnly = meta.story({
	render: () => (
		<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["list"]} aria-label="View mode">
			<Toggle render={<ToggleStyles />} value="list" aria-label="List view">
				<Icon name="list" size={14} />
			</Toggle>
			<Toggle render={<ToggleStyles />} value="tree" aria-label="Tree view">
				<Icon name="folder-tree" size={14} />
			</Toggle>
		</ToggleGroup>
	),
});

export const ThreeItems = meta.story({
	render: () => (
		<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["list"]} aria-label="View mode">
			<Toggle render={<ToggleStyles />} value="list">
				<Icon name="list" size={14} />
			</Toggle>
			<Toggle render={<ToggleStyles />} value="tree">
				<Icon name="folder-tree" size={14} />
			</Toggle>
			<Toggle render={<ToggleStyles />} value="grid">
				<Icon name="text-block" size={14} />
			</Toggle>
		</ToggleGroup>
	),
});
