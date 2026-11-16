import { stories } from "../stories/stories.ts";
import { StatusLed } from "./StatusLed.tsx";
import { Tooltip } from "./Tooltip.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Content & status/StatusLed",
	id: "components-statusled",
	component: StatusLed,
	parameters: { design: { type: "figma", url: core("2613-8797") } },
	args: { variant: "safe", "aria-label": "Online" },
});

export const Default = meta.story({ args: { variant: "safe", "aria-label": "Online" } });

const variants = [
	{ variant: "safe", label: "Online" },
	{ variant: "warning", label: "Reconnecting" },
	{ variant: "gray", label: "Offline" },
] as const;

/** Each light beside the word it stands for: up, up but needs a look, and not there. */
export const Variants = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 16 }}>
			{variants.map(({ variant, label }) => (
				<span
					key={variant}
					className="text-13"
					style={{ display: "flex", alignItems: "center", gap: 6 }}
				>
					<StatusLed variant={variant} aria-hidden />
					{label}
				</span>
			))}
		</div>
	),
});

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
							<StatusLed
								variant={online ? "safe" : "gray"}
								aria-label={online ? "Online" : "Offline"}
							/>
						</Tooltip>
					) : (
						<StatusLed
							variant={online ? "safe" : "gray"}
							aria-label={online ? "Online" : "Offline"}
						/>
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
				<StatusLed variant="safe" aria-hidden />
				live
			</span>
			<span className="text-12" style={{ display: "flex", alignItems: "center", gap: 6 }}>
				<StatusLed variant="safe" aria-hidden />3 of 4 online
			</span>
			<span className="text-12" style={{ display: "flex", alignItems: "center", gap: 6 }}>
				<StatusLed variant="gray" aria-hidden />
				offline
			</span>
		</div>
	),
});
