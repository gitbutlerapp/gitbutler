import { stories } from "../stories/stories.ts";
import { MachineStatus } from "./MachineStatus.tsx";
import { Tooltip } from "./Tooltip.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Workspace tree/MachineStatus",
	id: "components-machinestatus",
	component: MachineStatus,
	parameters: { design: { type: "figma", url: core("5303-109676") } },
	args: { online: true },
});

export const Default = meta.story({ args: { online: true } });

const machines = [
	{ name: "macbook", online: true },
	{ name: "closet-pc", online: true },
	{ name: "old-mac", online: false, seen: "2 days ago" },
];

/** Machines by name, each with its status. An offline one says in a tooltip when it was last seen. */
export const Machines = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
			{machines.map(({ name, online, seen }) => (
				<span
					key={name}
					className="text-14"
					style={{ display: "flex", alignItems: "center", gap: 6 }}
				>
					{name}
					{seen !== undefined ? (
						<Tooltip content={`Last seen ${seen}`}>
							<MachineStatus online={online} />
						</Tooltip>
					) : (
						<MachineStatus online={online} />
					)}
				</span>
			))}
		</div>
	),
});
