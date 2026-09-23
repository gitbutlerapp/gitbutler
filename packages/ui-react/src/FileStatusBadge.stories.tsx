import preview from "#storybook/preview";
import { FileStatusBadge, type FileStatusType } from "./FileStatusBadge.tsx";

const statuses: Array<FileStatusType> = ["Addition", "Deletion", "Modification", "Rename"];

const meta = preview.meta({
	component: FileStatusBadge,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2134-5945",
		},
	},
	argTypes: {
		status: {
			control: "select",
			options: statuses,
		},
	},
	args: {
		status: "Addition",
	},
});

export const Default = meta.story({
	args: {
		status: "Addition",
	},
});

export const AllStatuses = meta.story({
	args: {
		status: "Addition",
	},
	render: (args) => (
		<div style={{ display: "flex", gap: 16, alignItems: "center" }}>
			{statuses.map((status) => (
				<div
					key={status}
					style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "center" }}
				>
					<FileStatusBadge {...args} status={status} />
					<span style={{ fontSize: 11, opacity: 0.5 }}>{status}</span>
				</div>
			))}
		</div>
	),
});
