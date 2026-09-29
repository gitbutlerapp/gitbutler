import preview from "#storybook/preview";
import { useState } from "react";
import { Icon } from "./Icon.tsx";
import { Tab, Tabs } from "./Tabs.tsx";

const meta = preview.meta({
	component: Tabs,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2374-4526",
		},
	},
	decorators: [
		(Story) => (
			<div style={{ paddingBlock: 24 }}>
				<Story />
			</div>
		),
	],
});

/** Tabs that switch a pane's contents; the underline follows the selected one. */
export const Default = meta.story({
	render: () => (
		<Tabs defaultValue="diff" aria-label="Branch">
			<Tab value="diff">Diff</Tab>
			<Tab value="pr">Pull Request</Tab>
			<Tab value="sessions">Sessions</Tab>
			<Tab value="activity">Activity</Tab>
		</Tabs>
	),
});

/** With an `icon` before each label and a `count` after the ones that hold things. */
export const IconsAndCounts = meta.story({
	render: () => (
		<Tabs defaultValue="changes" aria-label="Worktree">
			<Tab value="changes" icon={<Icon name="diff" />} count={3}>
				Changes
			</Tab>
			<Tab value="pr" icon={<Icon name="pr" />}>
				Pull Request
			</Tab>
			<Tab value="sessions" icon={<Icon name="ai" />} count={2}>
				Sessions
			</Tab>
			<Tab value="activity" icon={<Icon name="history" />}>
				Activity
			</Tab>
		</Tabs>
	),
});

/** With `kbd`, the shortcut that selects each tab, faint after its label. */
export const Shortcuts = meta.story({
	render: () => (
		<Tabs defaultValue="diff" aria-label="Branch">
			<Tab value="diff" kbd="Mod+1">
				Diff
			</Tab>
			<Tab value="pr" kbd="Mod+2" count={4}>
				Pull Request
			</Tab>
			<Tab value="activity" kbd="Mod+3">
				Activity
			</Tab>
		</Tabs>
	),
});

/**
 * In a pane's header, controlled by the pane. The header leaves 12px below the row, the underline's
 * 8px gap and its 4px, so the underline sits on the divider rather than across it.
 */
export const InAHeader = meta.story({
	render: function Render() {
		const [tab, setTab] = useState("changes");
		return (
			<div style={{ width: 480, border: "1px solid var(--border-2)" }}>
				<div
					style={{
						paddingInline: 12,
						paddingBlock: "6px 12px",
						borderBlockEnd: "1px solid var(--border-2)",
					}}
				>
					<Tabs value={tab} onValueChange={setTab} aria-label="Worktree">
						<Tab value="changes" count={3}>
							Changes
						</Tab>
						<Tab value="activity">Activity</Tab>
					</Tabs>
				</div>
				<p className="text-13" style={{ padding: 12, margin: 0 }}>
					{tab === "changes" ? "Three changed files." : "Nothing happened yet."}
				</p>
			</div>
		);
	},
});
