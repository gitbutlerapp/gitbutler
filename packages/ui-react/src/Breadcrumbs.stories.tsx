import preview from "#storybook/preview";
import { useState } from "react";
import { Breadcrumbs, type Crumb } from "./Breadcrumbs.tsx";
import { MenuRadioGroup, MenuRadioItem } from "./Menu.tsx";

const meta = preview.meta({
	title: "Preview/Breadcrumbs",
	id: "components-breadcrumbs",
	component: Breadcrumbs,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2962-32845",
		},
	},
});

const worktree: Array<Crumb> = [
	{ label: "mesh", href: "#mesh" },
	{ label: "pavel--macbook-pro-2", href: "#machine" },
	{ label: "but-dev", href: "#repo" },
];

/** Where a page sits: the levels above it, a › between each. The header under it names the page. */
export const Default = meta.story({
	args: { items: worktree },
});

/**
 * Short of room, the levels between the first and the nearest fold into "…", which opens them as a
 * menu; the last level left ends in an ellipsis before the trail would spill.
 */
export const Narrow = meta.story({
	render: () => (
		<div style={{ width: 320 }}>
			<Breadcrumbs
				items={[
					...worktree,
					{ label: "Main worktree", href: "#worktree" },
					{ label: "ios-push-handoff", href: "#branch" },
				]}
			/>
		</div>
	),
});

/**
 * A level with a `menu`: its chevron opens the level and its siblings as a `MenuRadioGroup`, the
 * current one ticked, to jump to another machine.
 */
export const SiblingMenu = meta.story({
	render: function Render() {
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
				]}
			/>
		);
	},
});
