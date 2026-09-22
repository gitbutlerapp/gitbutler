import preview from "#storybook/preview";
import { FileStatusBadge, type FileStatusType } from "./FileStatusBadge.tsx";

const statuses: Array<FileStatusType> = ["Addition", "Deletion", "Modification", "Rename"];

const meta = preview.meta({
	component: FileStatusBadge,
	argTypes: {
		status: {
			control: "select",
			options: statuses,
		},
		fontSize: {
			control: { type: "range", min: 8, max: 64, step: 1 },
		},
	},
	args: {
		status: "Addition",
		fontSize: 11,
	},
});

export const Default = meta.story({
	args: {
		status: "Addition",
		fontSize: 10,
	},
});

export const AllStatuses = meta.story({
	args: {
		status: "Addition",
		fontSize: 11,
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
