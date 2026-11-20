import { useState, type FC, type ReactNode } from "react";
import { stories } from "../stories/stories.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { MetaCount, MetaStatus } from "./MetaCount.tsx";
import { EmptyState } from "./EmptyState.tsx";
import { Drawer, GroupLabel, RepoItem, TerminalItem } from "./SidebarRow.tsx";
import { Tooltip } from "./Tooltip.tsx";
import { treeGuides } from "./treeGuides.ts";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Sidebar/Drawer",
	id: "components-drawer",
	component: Drawer,
	parameters: { design: { type: "figma", url: core("5403-16821") }, width: 334 },
});

/** The card a drawer holds, on the panel's ground under its row. */
const Card: FC<{ children: ReactNode }> = ({ children }) => (
	<div
		style={{
			overflow: "hidden",
			border: "1px solid var(--but-border-subtle)",
			borderRadius: "var(--but-radius-card)",
			backgroundColor: "var(--but-background)",
		}}
	>
		{children}
	</div>
);

/** A drawer's card with nothing in it: the empty state, with the card's room around it. */
const EmptyCard: FC<{ children: ReactNode }> = ({ children }) => (
	<Card>
		<div style={{ padding: "28px 16px" }}>{children}</div>
	</Card>
);

/** One group of a drawer's card, ruled off from the next. */
const Group: FC<{ children: ReactNode; last?: boolean }> = ({ children, last = false }) => (
	<div
		style={{
			display: "flex",
			flexDirection: "column",
			gap: 1,
			padding: 8,
			borderBottom: last ? undefined : "1px solid var(--but-border-subtle)",
		}}
	>
		{children}
	</div>
);

type Terminal = { name: string; place: string; meta?: ReactNode };

/** Terminals under their group's label, on the guide that ties them to its chevron. */
const TerminalGroup: FC<{ label: string; terminals: Array<Terminal>; last?: boolean }> = ({
	label,
	terminals,
	last,
}) => {
	const guides = treeGuides([{ depth: 0 }, ...terminals.map(() => ({ depth: 1 }))]);
	return (
		<Group last={last}>
			<GroupLabel name={label} folded={false} />
			{terminals.map((terminal, index) => (
				<TerminalItem key={terminal.name} {...terminal} depth={1} guides={guides[index + 1]} />
			))}
		</Group>
	);
};

/** A ghost icon button at a drawer's end, with its tooltip. */
const ghost = (icon: "plus" | "mixer", label: string) => (
	<Tooltip key={label} content={label}>
		<Button variant="ghost" size="medium" iconOnly aria-label={label}>
			<Icon name={icon} />
		</Button>
	</Tooltip>
);

const blocked = (
	<Tooltip content="Command not found">
		<MetaStatus variant="failed" aria-label="Command not found" />
	</Tooltip>
);

/** Puts a project the sidebar hides back among its cards. */
const show = (name: string) => (
	<Tooltip content="Show project">
		<Button variant="outline" size="small" iconOnly aria-label={`Show ${name}`}>
			<Icon name="eye-closed" />
		</Button>
	</Tooltip>
);

/** The other projects, each in its own part of the card with its counts and a way back into the
 * sidebar. */
const Projects: FC = () => (
	<>
		<Group>
			<RepoItem
				name="but-dev"
				folded
				onToggleFolded={() => {}}
				meta={
					<>
						<MetaCount type="uncommitted">4</MetaCount>
						<MetaCount type="unpushed">2</MetaCount>
						<MetaCount type="branch">3</MetaCount>
						<MetaCount type="worktrees">2</MetaCount>
					</>
				}
				actions={show("but-dev")}
			/>
		</Group>
		<Group>
			<RepoItem
				name="gitbutler-docs"
				folded
				onToggleFolded={() => {}}
				meta={
					<>
						<MetaCount type="behind">7</MetaCount>
						<MetaCount type="branch">1</MetaCount>
					</>
				}
				actions={show("gitbutler-docs")}
			/>
		</Group>
		<Group last>
			<RepoItem
				name="homebrew-tap"
				folded
				onToggleFolded={() => {}}
				meta={
					<>
						<MetaCount type="unpushed">1</MetaCount>
						<MetaCount type="branch">2</MetaCount>
					</>
				}
				actions={show("homebrew-tap")}
			/>
		</Group>
	</>
);

/** A drawer, open: its row, and the card it holds under it. */
export const Default = meta.story({
	args: {
		name: "Other projects",
		count: 3,
		folded: false,
		children: (
			<Card>
				<Projects />
			</Card>
		),
	},
});

/** The terminals, grouped in their card by what they're at. */
export const GroupedTerminals = meta.story({
	parameters: { design: { type: "figma", url: core("5403-18924") } },
	args: {
		name: "Terminals",
		count: 3,
		folded: false,
		hoverActions: ghost("plus", "New terminal…"),
		children: (
			<Card>
				<TerminalGroup
					label="Idle"
					terminals={[
						{ name: "review-cards", place: "gpui-row-buttons" },
						{ name: "Terminal", place: "Main worktree" },
					]}
				/>
				<TerminalGroup
					label="Blocked"
					terminals={[{ name: "codex", place: "Main worktree", meta: blocked }]}
					last
				/>
			</Card>
		),
	},
});

/** A drawer that keeps whether it's open, as the host does. */
const Open: FC<
	Omit<Parameters<typeof Drawer>[0], "folded" | "onToggleFolded"> & { open?: boolean }
> = ({ open = false, ...props }) => {
	const [folded, setFolded] = useState(!open);
	return <Drawer {...props} folded={folded} onToggleFolded={() => setFolded(!folded)} />;
};

/**
 * The foot of the sidebar, under the cards: the other projects and the terminals, each a drawer.
 * A press anywhere on a drawer opens what it holds under it, but not on its buttons: Add project
 * always shows, Sort and show and New terminal… only under the pointer. While the projects are
 * open their card rules them off from the terminals, so the terminals drop their hairline.
 */
export const SidebarDrawers = meta.story({
	parameters: { design: { type: "figma", url: core("5376-12694") } },
	render: () => (
		<>
			<Open
				name="Other projects"
				count={24}
				open
				actions={ghost("plus", "Add project")}
				hoverActions={ghost("mixer", "Sort and show")}
			>
				<Card>
					<Projects />
				</Card>
			</Open>
			<Open name="Terminals" count={0} hoverActions={ghost("plus", "New terminal…")} last open>
				<EmptyCard>
					<EmptyState
						variant="tidy"
						title="No terminals open"
						description="New ones open in gitbutler / Main worktree"
					>
						<Button variant="gray" size="small">
							New terminal
							<Icon name="plus" />
						</Button>
					</EmptyState>
				</EmptyCard>
			</Open>
		</>
	),
});

/**
 * Both drawers with nothing in them. Every project already shows in the sidebar, so the projects
 * say so and offer another; the terminals say where a new one would open.
 */
export const Empty = meta.story({
	render: () => (
		<>
			<Open name="Other projects" count={0} open actions={ghost("plus", "Add project")}>
				<EmptyCard>
					<EmptyState
						variant="tidy"
						title="Every project is showing"
						description="Hide one and it waits here, or add another"
					>
						<Button variant="gray" size="small">
							Add project
							<Icon name="plus" />
						</Button>
					</EmptyState>
				</EmptyCard>
			</Open>
			<Open name="Terminals" count={0} hoverActions={ghost("plus", "New terminal…")} last open>
				<EmptyCard>
					<EmptyState
						variant="tidy"
						title="No terminals open"
						description="New ones open in gitbutler / Main worktree"
					>
						<Button variant="gray" size="small">
							New terminal
							<Icon name="plus" />
						</Button>
					</EmptyState>
				</EmptyCard>
			</Open>
		</>
	),
});
