import preview from "#storybook/preview";
import { useState, type FC } from "react";
import { Breadcrumbs, CrumbMenuButton, type Crumb } from "./Breadcrumbs.tsx";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { LocationBar, LocationBarDivider } from "./LocationBar.tsx";
import { MenuRadioGroup, MenuRadioItem } from "./Menu.tsx";
import { StatusLed } from "./StatusLed.tsx";
import { Tooltip } from "./Tooltip.tsx";

const meta = preview.meta({
	title: "Preview/LocationBar",
	id: "components-locationbar",
	component: LocationBar,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=3382-53021",
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

/** Mesh, machine, repo; the machine's ⌃⌄ jumps to the mesh's other machines. */
const Trail: FC<{ below?: Array<Crumb> }> = ({ below = [] }) => {
	const [machine, setMachine] = useState("pavel--macbook-pro-2");
	return (
		<Breadcrumbs
			items={[
				{ label: "mesh", href: "#mesh" },
				{
					label: machine,
					href: "#machine",
					menu: (
						<MenuRadioGroup value={machine} onValueChange={setMachine}>
							<MenuRadioItem value="pavel--macbook-pro-2">pavel--macbook-pro-2</MenuRadioItem>
							<MenuRadioItem value="studio-desktop">studio-desktop</MenuRadioItem>
							<MenuRadioItem value="mac-mini">mac-mini</MenuRadioItem>
						</MenuRadioGroup>
					),
				},
				{ label: "but-dev", href: "#repo" },
				...below,
			]}
		/>
	);
};

/** Its glyph narrows while the sidebar is folded away. */
const SidebarToggle: FC<{ hidden?: boolean }> = ({ hidden = false }) => (
	<Tooltip content="Toggle sidebar" kbd=".">
		<Button variant="ghost" iconOnly aria-label="Toggle sidebar" aria-pressed={!hidden}>
			<Icon name={hidden ? "sidebar-narrow" : "sidebar"} />
		</Button>
	</Tooltip>
);

const Back: FC = () => (
	<Tooltip content="Back">
		<Button variant="ghost" iconOnly aria-label="Back">
			<Icon name="arrow-left" />
		</Button>
	</Tooltip>
);

/** The repo's other checkouts, as but.dev draws them: how many, how many differ, and a ⌃⌄ to list them. */
const Elsewhere: FC = () => (
	<span style={{ display: "inline-flex", alignItems: "center", columnGap: 4 }}>
		Also in 2 places
		<span aria-hidden style={{ color: "var(--text-3)" }}>
			·
		</span>
		<span style={{ color: "var(--text-warn)" }}>1 differs</span>
		<CrumbMenuButton aria-label="Other places" />
	</span>
);

/** The machine's state, as but.dev draws it: a dot and a word, then Refresh. */
const Live: FC = () => (
	<>
		<span style={{ display: "inline-flex", alignItems: "center", columnGap: 6 }}>
			<StatusLed online aria-hidden />
			live
		</span>
		<Tooltip content="Refresh">
			<Button variant="ghost" iconOnly aria-label="Refresh">
				<Icon name="refresh" />
			</Button>
		</Tooltip>
	</>
);

/** With the sidebar showing: its toggle, where the page sits, and the machine's state. */
export const Default = meta.story({
	args: {
		leading: <SidebarToggle />,
		children: <Trail />,
		state: (
			<>
				<Elsewhere />
				<Live />
			</>
		),
	},
});

/** With the sidebar hidden, Back follows its toggle, so there is a way up without it. */
export const SidebarHidden = meta.story({
	args: {
		leading: (
			<>
				<SidebarToggle hidden />
				<LocationBarDivider />
				<Back />
			</>
		),
		children: <Trail />,
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
		children: <Trail below={[{ label: "Main worktree", href: "#worktree" }]} />,
		state: <Live />,
	},
});

/**
 * On a phone the view fills the screen and there is no sidebar beside it to toggle: Back alone
 * leads, the way out of the view.
 */
export const Phone = meta.story({
	decorators: [
		(Story) => (
			<div style={{ width: 375 }}>
				<Story />
			</div>
		),
	],
	args: {
		leading: <Back />,
		children: <Trail />,
		state: <Live />,
	},
});
