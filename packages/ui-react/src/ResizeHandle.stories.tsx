import preview from "#storybook/preview";
import type { FC } from "react";
import { Group, Panel } from "react-resizable-panels";
import { ResizeHandle } from "./ResizeHandle.tsx";

const Pane: FC<{ label: string }> = ({ label }) => (
	<div
		className="text-13"
		style={{ display: "grid", height: "100%", placeItems: "center", color: "var(--text-2)" }}
	>
		{label}
	</div>
);

const meta = preview.meta({
	component: ResizeHandle,
});

/** Between two side-by-side panels. Drag the hairline to resize them. */
export const Horizontal = meta.story({
	render: () => (
		<Group orientation="horizontal" style={{ height: 240, border: "1px solid var(--border-2)" }}>
			<Panel minSize={80}>
				<Pane label="Sidebar" />
			</Panel>
			<ResizeHandle />
			<Panel minSize={80}>
				<Pane label="Details" />
			</Panel>
		</Group>
	),
});

/** The same handle between stacked panels; it takes its axis from the group. */
export const Vertical = meta.story({
	render: () => (
		<Group orientation="vertical" style={{ height: 320, border: "1px solid var(--border-2)" }}>
			<Panel minSize={60}>
				<Pane label="Diff" />
			</Panel>
			<ResizeHandle />
			<Panel minSize={60}>
				<Pane label="Comments" />
			</Panel>
		</Group>
	),
});

/**
 * `grab="after"` puts the whole grab area past the hairline, so a scrollbar at the edge of the
 * panel before it stays reachable.
 */
export const GrabAfter = meta.story({
	render: () => (
		<Group orientation="horizontal" style={{ height: 240, border: "1px solid var(--border-2)" }}>
			<Panel minSize={80}>
				<Pane label="Sidebar" />
			</Panel>
			<ResizeHandle grab="after" />
			<Panel minSize={80}>
				<Pane label="Details" />
			</Panel>
		</Group>
	),
});
