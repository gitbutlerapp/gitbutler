import { useState, type FC, type ReactNode } from "react";
import { stories } from "../stories/stories.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { MetaCount, MetaStatus } from "./MetaCount.tsx";
import { EmptyState } from "./EmptyState.tsx";
import {
	BranchItem,
	CommitItem,
	Drawer,
	MoreItem,
	NavItem,
	RepoItem,
	RowDivider,
	UncommittedItem,
	WorktreeItem,
} from "./SidebarRow.tsx";
import { Column, push, RepoCard, TerminalGroup, update } from "./story-assets/workspaceTree.tsx";
import { Tooltip } from "./Tooltip.tsx";

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

/** A ghost icon button at a drawer's end, with its tooltip. */
const ghost = (icon: "plus" | "mixer", label: string) => (
	<Tooltip key={label} content={label}>
		<Button variant="ghost" iconOnly aria-label={label}>
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

/** The hidden projects, each in its own part of the card with its counts and a way back into the
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
		name: "Hidden projects",
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
 * The foot of the sidebar, under the cards: the hidden projects and the terminals, each a drawer.
 * A press anywhere on a drawer opens what it holds under it, but not on its buttons, which show
 * only under the pointer: Sort and show, Add project and New terminal…. The terminals keep their
 * hairline under the projects' card open.
 */
export const SidebarDrawers = meta.story({
	parameters: { design: { type: "figma", url: core("5376-12694") } },
	render: () => (
		<>
			<Open
				name="Hidden projects"
				count={24}
				open
				hoverActions={
					<>
						{ghost("mixer", "Sort and show")}
						{ghost("plus", "Add project")}
					</>
				}
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
						<Button variant="outline" size="medium">
							<Icon name="plus" />
							New terminal
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
			<Open name="Hidden projects" count={0} open hoverActions={ghost("plus", "Add project")}>
				<EmptyCard>
					<EmptyState
						variant="tidy"
						title="Every project is showing"
						description="Hide one and it waits here, or add another"
					>
						<Button variant="outline" size="medium">
							<Icon name="plus" />
							Add project
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
						<Button variant="outline" size="medium">
							<Icon name="plus" />
							New terminal
						</Button>
					</EmptyState>
				</EmptyCard>
			</Open>
		</>
	),
});

/** A part of the hidden projects' card: one project, ruled off from what's above it across the
 * card, as the drawer's projects are. */
const Part: FC<{ children: ReactNode }> = ({ children }) => (
	<div
		style={{
			display: "flex",
			flexDirection: "column",
			gap: 1,
			padding: 8,
			borderTop: "1px solid var(--but-border-subtle)",
		}}
	>
		{children}
	</div>
);

const age = (value: string) => <MetaCount type="age">{value}</MetaCount>;

/** The hidden projects listed folded, by what they hold, each with its way back into the sidebar. */
const hidden = [
	{
		name: "gitbutler-docs",
		meta: [
			["behind", "7"],
			["branch", "1"],
		],
	},
	{
		name: "homebrew-tap",
		meta: [
			["unpushed", "1"],
			["branch", "2"],
		],
	},
	{ name: "dnd-notes", meta: [["branch", "1"]] },
] as const;

/**
 * The hidden projects as the last card of the list, scrolling with the cards above it
 * rather than pinned at the panel's foot, where Terminals stays. A leading `NavItem` heads the card,
 * its chevron, its name and how many projects it holds, folded until opened, so it costs one row
 * while nobody looks. Open, a line across the card parts
 * one project from the next, and a project opened in place keeps its own `RowDivider` under its
 * `RepoItem`, so the two lines never read as the same.
 */
const HiddenCard: FC<{ open?: boolean; opened?: boolean }> = ({ open = false, opened = false }) => {
	const [folded, setFolded] = useState(!open);
	const [butDev, setButDev] = useState(opened);
	return (
		<section
			aria-label="Hidden projects"
			style={{
				flexShrink: 0,
				overflow: "hidden",
				border: "1px solid var(--but-border-subtle)",
				borderRadius: "var(--but-radius-card)",
				backgroundColor: "var(--but-background)",
			}}
		>
			<div style={{ padding: 8 }}>
				<NavItem
					leadingRow
					name="Hidden projects"
					icon="eye-closed"
					folded={folded}
					onToggleFolded={() => setFolded(!folded)}
					meta={<MetaCount type="repos">24</MetaCount>}
				/>
			</div>
			{!folded && (
				<>
					<Part>
						<RepoItem
							name="but-dev"
							folded={!butDev}
							onToggleFolded={() => setButDev(!butDev)}
							meta={
								butDev ? undefined : (
									<>
										<MetaCount type="uncommitted">4</MetaCount>
										<MetaCount type="unpushed">2</MetaCount>
										<MetaCount type="branch">3</MetaCount>
									</>
								)
							}
							actions={show("but-dev")}
						/>
						{butDev && (
							<>
								<RowDivider />
								<Column>
									<WorktreeItem main name="Main worktree" folded={false} meta={age("3d")} />
									<UncommittedItem count={4} depth={1} />
									<BranchItem title="review-cards" depth={1} folded actions={push(2)} />
									<BranchItem
										title="perf: cheaper diff stats"
										pr={{ number: 410, state: "open" }}
										depth={1}
										folded={false}
										meta={
											<>
												<MetaStatus variant="working">CI</MetaStatus>
												{age("8m")}
											</>
										}
									/>
									<CommitItem message="feat: 28px hunk separator" depth={2} pushed={false} />
									<CommitItem message="docs: handoff notes" depth={2} />
									<WorktreeItem name="review-ios" folded meta={age("1d")} actions={update(3)} />
								</Column>
							</>
						)}
					</Part>
					{hidden.map(({ name, meta: counts }) => (
						<Part key={name}>
							<RepoItem
								name={name}
								folded
								onToggleFolded={() => {}}
								meta={
									<>
										{counts.map(([type, value]) => (
											<MetaCount key={type} type={type}>
												{value}
											</MetaCount>
										))}
									</>
								}
								actions={show(name)}
							/>
						</Part>
					))}
					<Part>
						<MoreItem label="20 more" />
					</Part>
				</>
			)}
		</section>
	);
};

/** A sidebar column: the cards scrolling, the hidden projects' card last among them, and the
 * Terminals drawer pinned at the foot. */
const Column334: FC<{ children: ReactNode }> = ({ children }) => (
	<div style={{ display: "flex", flexDirection: "column", width: 334, height: 720 }}>
		<div
			style={{
				display: "flex",
				flexDirection: "column",
				gap: 8,
				flex: 1,
				minHeight: 0,
				overflowY: "auto",
				paddingBottom: 8,
			}}
		>
			<RepoCard
				label="berlin-brutalism"
				header={<RepoItem name="berlin-brutalism" folded={false} meta={age("3d")} />}
			>
				<BranchItem
					title="cursor-unique-names"
					folded
					meta={
						<>
							<MetaCount type="commits">23</MetaCount>
							{age("2h")}
						</>
					}
				/>
			</RepoCard>
			{children}
		</div>
		<Open name="Terminals" count={5} hoverActions={ghost("plus", "New terminal…")} last />
	</div>
);

/**
 * Mock: the hidden projects unpinned, the last card of the list, folded (left) and open with a
 * project opened in place (right). Terminals stays a drawer pinned at the foot.
 */
export const HiddenProjectsCard = meta.story({
	parameters: { width: undefined },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Column334>
				<HiddenCard />
			</Column334>
			<Column334>
				<HiddenCard open opened />
			</Column334>
		</div>
	),
});
