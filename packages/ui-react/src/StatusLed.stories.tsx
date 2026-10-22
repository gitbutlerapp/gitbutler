import preview from "#storybook/preview";
import { StatusLed } from "./StatusLed.tsx";
import { Tooltip } from "./Tooltip.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=${node}`;

const meta = preview.meta({
	title: "Content & status/StatusLed",
	id: "components-statusled",
	component: StatusLed,
	parameters: { design: { type: "figma", url: core("2613-8797") } },
	args: { online: true },
});

export const Default = meta.story({ args: { online: true } });

const machines = [
	{ name: "macbook", online: true },
	{ name: "closet-pc", online: true },
	{ name: "old-mac", online: false, seen: "2 days ago" },
];

/** Machines by name, each with its light. An offline one says in a tooltip when it was last seen. */
export const Machines = meta.story({
	parameters: { design: { type: "figma", url: core("4006-1538") } },
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
			{machines.map(({ name, online, seen }) => (
				<span
					key={name}
					className="text-13"
					style={{ display: "flex", alignItems: "center", gap: 6 }}
				>
					{seen !== undefined ? (
						<Tooltip content={`Last seen ${seen}`}>
							<StatusLed online={online} />
						</Tooltip>
					) : (
						<StatusLed online={online} />
					)}
					{name}
				</span>
			))}
		</div>
	),
});

/**
 * Beside words that already say whether the machines are online. The light is hidden from a
 * screen reader there, so it doesn't read "Online" before "live".
 */
export const WithItsWords = meta.story({
	parameters: { design: { type: "figma", url: core("4006-1551") } },
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
			<span className="text-12" style={{ display: "flex", alignItems: "center", gap: 6 }}>
				<StatusLed online aria-hidden />
				live
			</span>
			<span className="text-12" style={{ display: "flex", alignItems: "center", gap: 6 }}>
				<StatusLed online aria-hidden />3 of 4 online
			</span>
			<span className="text-12" style={{ display: "flex", alignItems: "center", gap: 6 }}>
				<StatusLed online={false} aria-hidden />
				offline
			</span>
		</div>
	),
});
