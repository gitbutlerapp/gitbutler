import preview from "#storybook/preview";
import { useState, type FC } from "react";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import {
	ContextMenu,
	Menu,
	MenuItem,
	MenuRadioGroup,
	MenuRadioItem,
	MenuSection,
	type MenuProps,
} from "./Menu.tsx";
import { MetaCount } from "./MetaCount.tsx";
import { BranchItem } from "./SidebarRow.tsx";

const meta = preview.meta({
	title: "Overlays/Menu",
	id: "components-menu",
	component: Menu,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2790-13820",
		},
	},
	args: {
		side: "bottom",
		align: "end",
		sideOffset: 4,
	},
	argTypes: {
		side: { control: "inline-radio", options: ["top", "bottom", "left", "right"] },
		align: { control: "inline-radio", options: ["start", "center", "end"] },
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "120px 48px 280px" }}>
				<Story />
			</div>
		),
	],
});

/**
 * A row's ⋯: its actions in sections, and one it can't run now held in place with the reason on
 * hover.
 */
export const Default = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2791-14242",
		},
	},
	args: {
		"aria-label": "Branch actions",
		trigger: (
			<Button variant="ghost" size="small" iconOnly aria-label="More actions">
				<Icon name="kebab" />
			</Button>
		),
		children: (
			<>
				<MenuSection>
					<MenuItem>Send to…</MenuItem>
					<MenuItem disabled hint="pavel--macbook-pro-2 is offline">
						Land…
					</MenuItem>
					<MenuItem>Rename…</MenuItem>
				</MenuSection>
				<MenuSection>
					<MenuItem>Archive</MenuItem>
					<MenuItem>Delete branch…</MenuItem>
				</MenuSection>
			</>
		),
	},
});

/** A header's view options, as the navigator holds them: three choices of one, each ticked where it stands. */
const ViewOptionsMenu: FC<MenuProps> = (props) => {
	const [group, setGroup] = useState("machine");
	const [sort, setSort] = useState("activity");
	const [show, setShow] = useState("active");
	return (
		<Menu
			{...props}
			aria-label="View options"
			trigger={
				<Button variant="ghost" iconOnly aria-label="View options">
					<Icon name="mixer" />
				</Button>
			}
		>
			<MenuRadioGroup label="Group by" value={group} onValueChange={setGroup}>
				<MenuRadioItem value="machine">Machine</MenuRadioItem>
				<MenuRadioItem value="repo">Repository</MenuRadioItem>
			</MenuRadioGroup>
			<MenuRadioGroup label="Sort by" value={sort} onValueChange={setSort}>
				<MenuRadioItem value="activity">Last activity</MenuRadioItem>
				<MenuRadioItem value="name">Name</MenuRadioItem>
			</MenuRadioGroup>
			<MenuRadioGroup label="Show" value={show} onValueChange={setShow}>
				<MenuRadioItem value="active">Active</MenuRadioItem>
				<MenuRadioItem value="archived">Archived</MenuRadioItem>
				<MenuRadioItem value="all">All</MenuRadioItem>
			</MenuRadioGroup>
		</Menu>
	);
};

/** A header's view options: three choices of one, each ticked where it stands. */
export const ViewOptions = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2791-14320",
		},
	},
	render: (args) => <ViewOptionsMenu {...args} />,
});

/** The same actions on a right-click, at the pointer. */
export const OnRightClick = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=2791-14441",
		},
	},
	render: () => (
		<ContextMenu
			aria-label="Commit actions"
			trigger={
				<div
					className="text-13"
					style={{
						padding: "24px 32px",
						border: "1px dashed var(--border-2)",
						borderRadius: "var(--radius-card)",
						color: "var(--text-2)",
					}}
				>
					Right-click here
				</div>
			}
		>
			<MenuSection>
				<MenuItem icon="copy" kbd="Mod+C">
					Copy SHA
				</MenuItem>
				<MenuItem icon="edit">Edit message…</MenuItem>
			</MenuSection>
			<MenuSection>
				<MenuItem icon="undo">Uncommit</MenuItem>
			</MenuSection>
		</ContextMenu>
	),
});

/** A branch's actions, the same from its ⋯ and from a right-click. */
const branchActions = (
	<>
		<MenuSection>
			<MenuItem>Send to…</MenuItem>
			<MenuItem>Copy branch name</MenuItem>
		</MenuSection>
		<MenuSection>
			<MenuItem>Rename…</MenuItem>
			<MenuItem>Delete branch…</MenuItem>
		</MenuSection>
	</>
);

/**
 * A row's actions, from its ⋯ and from a right-click anywhere on it: the ⋯ is a `Menu` in the
 * row's `menu` slot, and the row itself is a `ContextMenu`'s trigger, both with the same rows.
 * Each menu keeps its own state, so a right-click leaves the ⋯ unpressed.
 */
export const OnARow = meta.story({
	render: () => (
		<div style={{ width: 334 }}>
			<ContextMenu
				aria-label="Branch actions"
				trigger={
					<BranchItem
						title="resume-on-laptop"
						folded
						meta={<MetaCount type="unpushed">9</MetaCount>}
						menu={
							<Menu
								aria-label="Branch actions"
								trigger={
									<Button variant="ghost" size="small" iconOnly aria-label="More actions">
										<Icon name="kebab" />
									</Button>
								}
							>
								{branchActions}
							</Menu>
						}
					/>
				}
			>
				{branchActions}
			</ContextMenu>
		</div>
	),
});
