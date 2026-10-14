import preview from "#storybook/preview";
import type { FC } from "react";
import { Breadcrumbs, type Crumb } from "./Breadcrumbs.tsx";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { LocationBar, LocationBarDivider } from "./LocationBar.tsx";
import { Tooltip } from "./Tooltip.tsx";

const meta = preview.meta({
	title: "Preview/LocationBar",
	id: "components-locationbar",
	component: LocationBar,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=3069-37473",
		},
	},
	decorators: [
		// The top of the details pane, at the width it usually has.
		(Story) => (
			<div style={{ width: 820 }}>
				<Story />
			</div>
		),
	],
});

const trail: Array<Crumb> = [
	{ label: "mesh", href: "#mesh" },
	{ label: "pavel--macbook-pro-2", href: "#machine" },
	{ label: "but-dev", href: "#repo" },
];

const SidebarToggle: FC = () => (
	<Tooltip side="bottom" content="Toggle sidebar" kbd=".">
		<Button variant="ghost" size="small" iconOnly aria-label="Toggle sidebar">
			<Icon name="sidebar" />
		</Button>
	</Tooltip>
);

/** The machine's state, as but.dev draws it: a dot and a word, then Refresh. */
const Live: FC = () => (
	<>
		<span style={{ display: "inline-flex", alignItems: "center", columnGap: 6 }}>
			<span
				aria-hidden
				style={{ width: 6, height: 6, borderRadius: "50%", backgroundColor: "var(--text-safe)" }}
			/>
			live
		</span>
		<Button size="small">Refresh</Button>
	</>
);

/** With the sidebar showing: its toggle, where the page sits, and the machine's state. */
export const Default = meta.story({
	args: {
		leading: <SidebarToggle />,
		children: <Breadcrumbs items={trail} />,
		state: <Live />,
	},
});

/** With the sidebar hidden, Back follows its toggle, so there is a way up without it. */
export const SidebarHidden = meta.story({
	args: {
		leading: (
			<>
				<SidebarToggle />
				<LocationBarDivider />
				<Tooltip side="bottom" content="Back">
					<Button variant="ghost" size="small" iconOnly aria-label="Back">
						<Icon name="arrow-left" />
					</Button>
				</Tooltip>
			</>
		),
		children: <Breadcrumbs items={trail} />,
		state: <Live />,
	},
});

/** In a narrow pane the state keeps its size and the trail folds into "…". */
export const Narrow = meta.story({
	decorators: [
		(Story) => (
			<div style={{ width: 420 }}>
				<Story />
			</div>
		),
	],
	args: {
		leading: <SidebarToggle />,
		children: <Breadcrumbs items={[...trail, { label: "Main worktree", href: "#worktree" }]} />,
		state: <Live />,
	},
});
