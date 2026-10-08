import preview from "#storybook/preview";
import type { FC, ReactNode } from "react";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { CiStatus, MetaCount } from "./MetaCount.tsx";
import {
	BranchItem,
	CommitItem,
	MachineItem,
	MoreItem,
	RepoItem,
	StackCaption,
	UncommittedItem,
	WorktreeDivider,
	WorktreeItem,
	type SidebarRowLayout,
} from "./SidebarRow.tsx";
import { machinePictures } from "./story-assets/machines.ts";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=${node}`;

const meta = preview.meta({
	component: BranchItem,
	parameters: { design: { type: "figma", url: core("2572-3458") } },
	argTypes: {
		layout: { control: "inline-radio", options: ["compact", "rich"] },
	},
	args: {
		title: "fix: retry stalled transfers",
		pr: { number: 377, state: "open" },
		branch: "retry-transfers",
		layout: "rich",
		depth: 1,
		folded: true,
		selected: false,
	},
	decorators: [
		(Story) => (
			<div style={{ width: 316, paddingBlock: 24 }}>
				<Story />
			</div>
		),
	],
});

const push = (
	<Button size="small" key="push">
		Push
	</Button>
);
const update = (
	<Button size="small" key="update">
		Update
	</Button>
);
const viewPr = (
	<Button size="small" iconOnly aria-label="Open pull request" key="pr">
		<Icon name="arrow-up-right" />
	</Button>
);
const menu = (what: string) => (
	<Button variant="ghost" size="small" iconOnly aria-label={`${what} menu`}>
		<Icon name="kebab" />
	</Button>
);

/**
 * A branch with an open pull request. In `rich` it takes two lines: the request's title over the
 * branch's name, the checks, what is still to push, and the next step; the menu shows under the
 * pointer. Switch `layout` to `compact` for one line, the state at the end.
 */
export const Default = meta.story({
	args: { title: "fix: retry stalled transfers" },
	render: (args) => (
		<BranchItem
			{...args}
			meta={
				<>
					<CiStatus status="working" />
					<MetaCount type="unpushed">4</MetaCount>
				</>
			}
			actions={
				<>
					{push}
					{viewPr}
				</>
			}
			menu={menu("Branch")}
		/>
	),
});

/**
 * A sidebar card: a machine's header over its repositories, as the stories below build them. A
 * section of the page, named for its machine, so a screen reader can move between cards.
 */
const Card: FC<{ children: ReactNode; focused?: boolean; header: ReactNode; label: string }> = ({
	children,
	focused = false,
	header,
	label,
}) => (
	<section
		aria-label={label}
		style={{
			width: 334,
			flexShrink: 0,
			overflow: "hidden",
			border: "1px solid var(--border-section)",
			borderRadius: "var(--radius-card)",
			backgroundColor: "var(--bg-1)",
		}}
		data-selection-focused={focused ? "true" : undefined}
	>
		<div style={{ padding: 8, borderBottom: "1px solid var(--border-section)" }}>{header}</div>
		{children}
	</section>
);

/** One repository's rows in a card, ruled off from the next. */
const Repository: FC<{ children: ReactNode; last?: boolean }> = ({ children, last = false }) => (
	<div
		style={{
			display: "flex",
			flexDirection: "column",
			gap: 1,
			padding: 8,
			borderBottom: last ? undefined : "1px solid var(--border-section)",
		}}
	>
		{children}
	</div>
);

const machine = (
	<MachineItem
		name="pavel--macbook-pro-2"
		online
		folded={false}
		icon={<img src={machinePictures[0].src} alt="" width={14} height={14} />}
		menu={menu("Machine")}
	/>
);

/** The same machine in either layout: two repositories, one with two worktrees and a stack. */
const Tree: FC<{ layout: SidebarRowLayout; selected?: string }> = ({ layout, selected }) => {
	const rich = layout === "rich";
	const age = (value: string) => (rich ? undefined : <MetaCount type="age">{value}</MetaCount>);
	return (
		<>
			<Repository>
				<RepoItem layout={layout} name="but-dev" folded={false} menu={menu("Repository")} />
				<WorktreeItem
					layout={layout}
					main
					name="Main worktree"
					depth={1}
					folded={false}
					meta={
						<>
							<MetaCount type="behind">4</MetaCount>
							{age("1d")}
						</>
					}
					actions={update}
					menu={menu("Worktree")}
				/>
				<UncommittedItem
					layout={layout}
					count={19}
					depth={2}
					selected={selected === "uncommitted"}
					menu={menu("Uncommitted changes")}
				/>
				<BranchItem
					layout={layout}
					title="ios-push-handoff"
					depth={2}
					folded
					selected={selected === "ios-push-handoff"}
					meta={
						<>
							<MetaCount type="unpushed">7</MetaCount>
							{age("1d")}
						</>
					}
					actions={push}
					menu={menu("Branch")}
				/>
				<WorktreeDivider depth={2} />
				<StackCaption label="Stack of 2" depth={2} menu={menu("Stack")} />
				<BranchItem
					layout={layout}
					title="fix: retry stalled transfers"
					pr={{ number: 377, state: "open" }}
					branch="retry-transfers"
					depth={2}
					folded
					meta={
						<>
							<CiStatus status="working" />
							<MetaCount type="unpushed">4</MetaCount>
							{age("8m")}
						</>
					}
					actions={
						<>
							{push}
							{viewPr}
						</>
					}
					menu={menu("Branch")}
				/>
				<BranchItem
					layout={layout}
					title="feat: resume work on another machine"
					pr={{ number: 280, state: "open" }}
					branch="resume-work"
					depth={2}
					folded={false}
					meta={
						<>
							<CiStatus status="working" />
							<MetaCount type="unpushed">7</MetaCount>
						</>
					}
					actions={
						<>
							{push}
							{viewPr}
						</>
					}
					menu={menu("Branch")}
				/>
				<CommitItem
					message="chore: separator tokens"
					pushed={false}
					depth={3}
					menu={menu("Commit")}
				/>
				<CommitItem
					message="feat: 28px hunk separator"
					depth={3}
					selected={selected === "commit"}
					menu={menu("Commit")}
				/>
				<CommitItem message="feat: elapsed time for agents" depth={3} menu={menu("Commit")} />
				<WorktreeDivider depth={2} />
				<BranchItem
					layout={layout}
					title="chore: bump dependencies"
					pr={{ number: 292, state: "merged" }}
					depth={2}
					folded
					meta={
						<>
							<MetaCount type="commits">12</MetaCount>
							<MetaCount type="age">8m</MetaCount>
						</>
					}
					menu={menu("Branch")}
				/>
				<MoreItem label="3 more" depth={2} />
				<WorktreeDivider depth={1} />
				<WorktreeItem
					layout={layout}
					name="mesh-archive"
					depth={1}
					folded={false}
					meta={
						<>
							<MetaCount type="behind">2</MetaCount>
							{age("5h")}
						</>
					}
					actions={update}
					menu={menu("Worktree")}
				/>
				<UncommittedItem layout={layout} count={2} depth={2} menu={menu("Uncommitted changes")} />
				<BranchItem
					layout={layout}
					title="resume-on-laptop"
					depth={2}
					folded
					meta={
						<>
							<MetaCount type="unpushed">9</MetaCount>
							{age("5h")}
						</>
					}
					actions={push}
					menu={menu("Branch")}
				/>
			</Repository>
			<Repository last>
				<RepoItem
					layout={layout}
					name="berlin-brutalism"
					folded={false}
					meta={
						<>
							<MetaCount type="behind">8</MetaCount>
							{age("2h")}
						</>
					}
					actions={update}
					menu={menu("Repository")}
				/>
				<BranchItem
					layout={layout}
					title="daemon-idle-cpu"
					depth={1}
					folded
					meta={
						<>
							<MetaCount type="commits">27</MetaCount>
							{age("5h")}
						</>
					}
					actions={<Button size="small">Create PR</Button>}
					menu={menu("Branch")}
				/>
			</Repository>
		</>
	);
};

/**
 * `compact`: one line a row, its state at the end. Under the pointer, on focus and on a selected
 * row, the actions and the menu take the state's place.
 */
export const Compact = meta.story({
	parameters: { design: { type: "figma", url: core("2594-15724") } },
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Tree layout="compact" />
		</Card>
	),
});

/**
 * `rich`: the state and the next step stay in view. A pull request takes two lines, its title
 * wrapping once under its glyph; every other row stays one line, its state and actions right after
 * its name. A merged request stays one muted line.
 */
export const Rich = meta.story({
	parameters: { design: { type: "figma", url: core("2642-12033") } },
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Tree layout="rich" />
		</Card>
	),
});

/**
 * A selected row: the solid fill while the list holds the selection focus, inverting what it
 * shows, and the quiet one in a list that has lost it. A selected row shows its menu, as the
 * pointer would.
 */
export const Selection = meta.story({
	parameters: { design: { type: "figma", url: core("2642-13680") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Card label="pavel--macbook-pro-2" header={machine} focused>
				<Tree layout="rich" selected="commit" />
			</Card>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<Tree layout="rich" selected="ios-push-handoff" />
			</Card>
		</div>
	),
});

/** A machine that is offline: its light goes grey and its picture dims. */
export const OfflineMachine = meta.story({
	parameters: { design: { type: "figma", url: core("2579-7658") } },
	render: () => (
		<MachineItem
			name="studio-desktop"
			online={false}
			folded
			icon={<img src={machinePictures[0].src} alt="" width={14} height={14} />}
			meta={
				<>
					<MetaCount type="repos">4</MetaCount>
					<MetaCount type="age">2d</MetaCount>
				</>
			}
		/>
	),
});

/**
 * A worktree's uncommitted changes as a row of their own, first among its branches: straight under
 * a repository with one worktree, and under each worktree of one with several. The repository and
 * the worktree no longer carry the ± count while open; this row does, and opens the changes.
 */
export const UncommittedChanges = meta.story({
	parameters: { design: { type: "figma", url: core("2572-3687") } },
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 1 }}>
			<RepoItem
				name="berlin-brutalism"
				folded={false}
				meta={
					<>
						<MetaCount type="behind">1</MetaCount>
						<MetaCount type="age">2h</MetaCount>
					</>
				}
				menu={menu("Repository")}
			/>
			<UncommittedItem count={12} depth={1} menu={menu("Uncommitted changes")} />
			<BranchItem
				title="cursor-unique-names"
				depth={1}
				folded
				meta={
					<>
						<MetaCount type="commits">23</MetaCount>
						<MetaCount type="age">2h</MetaCount>
					</>
				}
				menu={menu("Branch")}
			/>
			<WorktreeDivider depth={0} />
			<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
			<WorktreeItem
				name="review-ios"
				depth={1}
				folded={false}
				meta={
					<>
						<MetaCount type="behind">8</MetaCount>
						<MetaCount type="age">40m</MetaCount>
					</>
				}
				menu={menu("Worktree")}
			/>
			<UncommittedItem count={17} depth={2} selected menu={menu("Uncommitted changes")} />
			<BranchItem
				title="ios-push-handoff"
				depth={2}
				folded
				meta={<MetaCount type="unpushed">4</MetaCount>}
				menu={menu("Branch")}
			/>
		</div>
	),
});

const picture = (index: number) => (
	<img src={machinePictures[index]?.src} alt="" width={14} height={14} />
);

/** One repository's card when the sidebar is grouped by repository: its machines, folded or open. */
const ByRepositoryTree: FC<{ layout: SidebarRowLayout; open: boolean }> = ({ layout, open }) => {
	const rich = layout === "rich";
	const age = (value: string) => (rich ? undefined : <MetaCount type="age">{value}</MetaCount>);
	return (
		<Repository last>
			<MachineItem
				name="pavel--macbook-pro-2"
				online
				folded={!open}
				icon={picture(0)}
				meta={
					open ? undefined : (
						<>
							<MetaCount type="uncommitted">12</MetaCount>
							<MetaCount type="unpushed">8</MetaCount>
							<MetaCount type="behind">3</MetaCount>
							<MetaCount type="worktrees">2</MetaCount>
							<MetaCount type="age">8m</MetaCount>
						</>
					)
				}
				menu={menu("Machine")}
			/>
			{open && (
				<>
					<WorktreeItem
						layout={layout}
						main
						name="Main worktree"
						depth={1}
						folded={false}
						meta={
							<>
								<MetaCount type="behind">3</MetaCount>
								{age("8m")}
							</>
						}
						actions={update}
						menu={menu("Worktree")}
					/>
					<UncommittedItem
						layout={layout}
						count={12}
						depth={2}
						menu={menu("Uncommitted changes")}
					/>
					<BranchItem
						layout={layout}
						title="mesh-archive"
						depth={2}
						folded
						meta={
							<>
								<MetaCount type="unpushed">4</MetaCount>
								<MetaCount type="agent-working">12m</MetaCount>
							</>
						}
						actions={push}
						menu={menu("Branch")}
					/>
					<BranchItem
						layout={layout}
						title="fix: redact remote credentials"
						pr={{ number: 32, state: "open" }}
						branch="redact-remote-creds"
						depth={2}
						folded
						meta={
							<>
								<CiStatus status="working" />
								<MetaCount type="unpushed">4</MetaCount>
								{age("3d")}
							</>
						}
						actions={
							<>
								{push}
								{viewPr}
							</>
						}
						menu={menu("Branch")}
					/>
					<WorktreeDivider depth={1} />
					<WorktreeItem
						layout={layout}
						name="review-ios"
						depth={1}
						folded
						meta={<MetaCount type="age">1d</MetaCount>}
						menu={menu("Worktree")}
					/>
				</>
			)}
			<MachineItem
				name="studio-desktop"
				online={false}
				folded={!open}
				icon={picture(6)}
				meta={
					open ? (
						<>
							<MetaCount type="behind">3</MetaCount>
							<MetaCount type="age">2d</MetaCount>
						</>
					) : (
						<>
							<MetaCount type="unpushed">4</MetaCount>
							<MetaCount type="behind">3</MetaCount>
							<MetaCount type="age">2d</MetaCount>
						</>
					)
				}
				actions={update}
				menu={menu("Machine")}
			/>
			{open && (
				<>
					<BranchItem
						layout={layout}
						title="resume-on-laptop"
						depth={1}
						folded
						meta={
							<>
								<MetaCount type="unpushed">4</MetaCount>
								{age("2d")}
							</>
						}
						actions={push}
						menu={menu("Branch")}
					/>
					<BranchItem
						layout={layout}
						title="feat: archive old worktrees"
						pr={{ number: 31, state: "merged" }}
						depth={1}
						folded
						meta={
							<>
								<MetaCount type="commits">5</MetaCount>
								<MetaCount type="age">3d</MetaCount>
							</>
						}
						menu={menu("Branch")}
					/>
				</>
			)}
			<MachineItem
				name="cloud"
				online
				folded
				icon={<Icon name="cloud" />}
				meta={
					<>
						<MetaCount type="worktrees">3</MetaCount>
						<MetaCount type="agent-waiting">2m</MetaCount>
					</>
				}
				menu={menu("Machine")}
			/>
		</Repository>
	);
};

const repository = <RepoItem name="but-dev" folded={false} menu={menu("Repository")} />;

/**
 * Grouped by repository: the repository heads the card and its machines are rows inside it, each
 * over its own worktrees, or straight over the branches of its only one. Folded, a machine sums up
 * its worktrees and its age, and a cloud machine shows its agents' time. Open, a row over several
 * worktrees shows no age, as each says its own. The same card in `compact` and in `rich`.
 */
export const ByRepository = meta.story({
	parameters: { design: { type: "figma", url: core("2642-10912") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Card label="but-dev" header={repository}>
				<ByRepositoryTree layout="compact" open={false} />
			</Card>
			<Card label="but-dev" header={repository}>
				<ByRepositoryTree layout="compact" open />
			</Card>
			<Card label="but-dev" header={repository}>
				<ByRepositoryTree layout="rich" open />
			</Card>
		</div>
	),
});
