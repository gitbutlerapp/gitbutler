import { stories } from "../docs/stories.ts";
import { Button } from "./Button.tsx";
import { DescriptionList, DescriptionListItem } from "./DescriptionList.tsx";
import { Icon } from "./Icon.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
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

/** Flags with what each does: `code` shows a name as it is typed. */
export const Flags = meta.story({
	parameters: figma("4690-1017"),
	render: () => (
		<DescriptionList>
			<DescriptionListItem label="--dirty" code>
				Include uncommitted changes
			</DescriptionListItem>
			<DescriptionListItem label="--agent codex" code>
				Use Codex, with OPENAI_API_KEY
			</DescriptionListItem>
			<DescriptionListItem label="--for 2h" code>
				Run past the default 45 minutes
			</DescriptionListItem>
		</DescriptionList>
	),
});

/** Environment variables to set: one with the value to paste, one with where to find it. */
export const Variables = meta.story({
	parameters: figma("4690-1027"),
	render: () => (
		<DescriptionList>
			<DescriptionListItem label="VERCEL_TEAM_ID" code onCopy={copyToClipboard}>
				team_8d03b6
			</DescriptionListItem>
			<DescriptionListItem label="VERCEL_TOKEN" code>
				From your Vercel account settings
			</DescriptionListItem>
		</DescriptionList>
	),
});
