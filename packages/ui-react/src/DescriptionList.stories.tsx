import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { DescriptionList, DescriptionListItem } from "./DescriptionList.tsx";
import { Icon } from "./Icon.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = preview.meta({
	title: "Content & status/DescriptionList",
	id: "components-descriptionlist",
	component: DescriptionList,
	parameters: figma("4629-789"),
	decorators: [
		(Story) => (
			<div style={{ maxWidth: 386 }}>
				<Story />
			</div>
		),
	],
});

const copyToClipboard = (value: string) => void navigator.clipboard.writeText(value);

export const Default = meta.story({
	args: {
		children: (
			<>
				<DescriptionListItem label="Type">Custom</DescriptionListItem>
				<DescriptionListItem label="Allowed domains" onCopy={copyToClipboard}>
					but.dev
				</DescriptionListItem>
			</>
		),
	},
});

/** Values to copy, a long name that wraps, and a button as a value. */
export const ApiCredential = meta.story({
	parameters: figma("4629-2038"),
	render: () => (
		<DescriptionList>
			<DescriptionListItem label="Type">Bearer</DescriptionListItem>
			<DescriptionListItem label="Allowed websites" onCopy={copyToClipboard}>
				but.dev
			</DescriptionListItem>
			<DescriptionListItem label="Replace Authorization" onCopy={copyToClipboard}>
				X-But-Join-Key
			</DescriptionListItem>
			<DescriptionListItem label="Prefix">Clear Bearer and leave blank</DescriptionListItem>
			<DescriptionListItem label="Value">
				<Button>
					Create join key
					<Icon name="key" />
				</Button>
			</DescriptionListItem>
		</DescriptionList>
	),
});
