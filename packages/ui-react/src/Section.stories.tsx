import { stories } from "../docs/stories.ts";
import { CodeBlock } from "./CodeBlock.tsx";
import { DescriptionList, DescriptionListItem } from "./DescriptionList.tsx";
import { Section } from "./Section.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/Section",
	id: "components-section",
	component: Section,
	parameters: figma("4630-892"),
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
		title: "Network access",
		children: (
			<DescriptionList>
				<DescriptionListItem label="Type">Custom</DescriptionListItem>
				<DescriptionListItem label="Allowed domains" onCopy={copyToClipboard}>
					but.dev
				</DescriptionListItem>
			</DescriptionList>
		),
	},
});

/** The settings for a Claude Code cloud environment, as the connect modal lists them. */
export const CloudSessionSettings = meta.story({
	parameters: figma("4630-916"),
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
			<Section title="Network access">
				<DescriptionList>
					<DescriptionListItem label="Type">Custom</DescriptionListItem>
					<DescriptionListItem label="Allowed domains" onCopy={copyToClipboard}>
						but.dev
					</DescriptionListItem>
				</DescriptionList>
			</Section>
			<Section title="API credential">
				<DescriptionList>
					<DescriptionListItem label="Type">Bearer</DescriptionListItem>
					<DescriptionListItem label="Replace Authorization" onCopy={copyToClipboard}>
						X-But-Join-Key
					</DescriptionListItem>
				</DescriptionList>
			</Section>
			<Section title="Setup script">
				<CodeBlock onCopy={copyToClipboard}>
					{
						"curl -fsSL https://but.dev/install | BUT_MESH_NO_LOGIN=1 sh && ~/.local/bin/but-mesh join --install-cloud-hook"
					}
				</CodeBlock>
			</Section>
		</div>
	),
});
