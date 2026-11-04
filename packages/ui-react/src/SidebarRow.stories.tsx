import { stories } from "../docs/stories.ts";
import { useState, type FC, type ReactNode } from "react";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { CiStatus, MetaCount } from "./MetaCount.tsx";
import {
	BranchItem,
	CommitItem,
	GroupLabel,
	MachineItem,
	MoreItem,
	NavItem,
	NoticeItem,
	RepoItem,
	StackCaption,
	TargetItem,
	UncommittedItem,
	WorktreeDivider,
	WorktreeItem,
} from "./SidebarRow.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=${node}`;

const meta = stories({
	title: "Sidebar/SidebarRow",
	id: "components-sidebarrow",
	component: BranchItem,
	parameters: { design: { type: "figma", url: core("2572-3458") } },
	args: {
		title: "fix: retry stalled transfers",
		pr: { number: 377, state: "open" },
		merged: false,
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
 * A branch with an open pull request: its checks and what is still to push at the row's end. Under
 * the pointer the next step and the menu take their place.
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
		icon={<Icon name="laptop" />}
		menu={menu("Machine")}
	/>
);

/** A machine: two repositories, one with two worktrees and a stack. */
const Tree: FC<{ selected?: string }> = ({ selected }) => {
	const age = (value: string) => <MetaCount type="age">{value}</MetaCount>;
	const [stackFolded, setStackFolded] = useState(false);
	return (
		<>
			<Repository>
				<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
				<WorktreeItem
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
					count={19}
					depth={2}
					selected={selected === "uncommitted"}
					menu={menu("Uncommitted changes")}
				/>
				<BranchItem
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
				<StackCaption
					count={2}
					tip="fix: retry stalled transfers"
					depth={2}
					folded={stackFolded}
					onToggleFolded={() => setStackFolded(!stackFolded)}
					meta={
						stackFolded && (
							<>
								<MetaCount type="unpushed">11</MetaCount>
								<MetaCount type="age">8m</MetaCount>
							</>
						)
					}
					menu={menu("Stack")}
				/>
				{!stackFolded && (
					<>
						<BranchItem
							title="fix: retry stalled transfers"
							pr={{ number: 377, state: "open" }}
							depth={3}
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
							title="feat: resume work on another machine"
							pr={{ number: 280, state: "open" }}
							depth={3}
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
							depth={4}
							menu={menu("Commit")}
						/>
						<CommitItem
							message="feat: 28px hunk separator"
							depth={4}
							selected={selected === "commit"}
							menu={menu("Commit")}
						/>
						<CommitItem message="feat: elapsed time for agents" depth={4} menu={menu("Commit")} />
					</>
				)}
				<BranchItem
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
				<UncommittedItem count={2} depth={2} menu={menu("Uncommitted changes")} />
				<BranchItem
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
				<BranchItem
					title="offline-cache"
					depth={2}
					folded="empty"
					meta={<MetaCount type="empty">empty</MetaCount>}
					menu={menu("Branch")}
				/>
			</Repository>
			<Repository last>
				<RepoItem
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
 * Grouped by machine: one line a row, its state at the end. Under the pointer, on focus and on a
 * selected row, the actions and the menu take the state's place.
 */
export const ByMachine = meta.story({
	parameters: { design: { type: "figma", url: core("2594-15724") } },
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Tree />
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
				<Tree selected="commit" />
			</Card>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<Tree selected="ios-push-handoff" />
			</Card>
		</div>
	),
});

/** A stack's caption over its branches, which it hides while folded and sums up instead. */
const Stack: FC<{
	tip: string;
	branches: Array<ReactNode>;
	summary: ReactNode;
	initiallyFolded: boolean;
}> = ({ tip, branches, summary, initiallyFolded }) => {
	const [folded, setFolded] = useState(initiallyFolded);
	return (
		<>
			<StackCaption
				count={branches.length}
				tip={tip}
				depth={1}
				folded={folded}
				onToggleFolded={() => setFolded(!folded)}
				meta={folded && summary}
				menu={menu("Stack")}
			/>
			{!folded && branches}
		</>
	);
};

/** One repository's branches: two stacks around a branch of its own. */
const Stacks: FC<{ folded: [boolean, boolean] }> = ({ folded }) => (
	<Repository last>
		<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
		<UncommittedItem count={10} depth={1} menu={menu("Uncommitted changes")} />
		<BranchItem
			title="mesh-archive"
			depth={1}
			folded
			meta={
				<>
					<MetaCount type="unpushed">2</MetaCount>
					<MetaCount type="age">1d</MetaCount>
				</>
			}
			actions={push}
			menu={menu("Branch")}
		/>
		<Stack
			tip="feat: archive UI"
			initiallyFolded={folded[0]}
			summary={
				<>
					<MetaCount type="unpushed">5</MetaCount>
					<MetaCount type="age">12m</MetaCount>
				</>
			}
			branches={[
				<BranchItem
					key="ui"
					title="feat: archive UI"
					pr={{ number: 34, state: "open" }}
					depth={2}
					folded
					meta={
						<>
							<CiStatus status="working" />
							<MetaCount type="unpushed">2</MetaCount>
							<MetaCount type="age">12m</MetaCount>
						</>
					}
					actions={push}
					menu={menu("Branch")}
				/>,
				<BranchItem
					key="api"
					title="archive-api"
					depth={2}
					folded
					meta={
						<>
							<MetaCount type="unpushed">3</MetaCount>
							<MetaCount type="age">1h</MetaCount>
						</>
					}
					actions={push}
					menu={menu("Branch")}
				/>,
				<BranchItem
					key="schema"
					title="feat: archive schema"
					pr={{ number: 31, state: "open" }}
					depth={2}
					folded
					meta={
						<>
							<CiStatus status="working" />
							<MetaCount type="age">5h</MetaCount>
						</>
					}
					menu={menu("Branch")}
				/>,
			]}
		/>
		<BranchItem
			title="fix: redact remote credentials"
			pr={{ number: 32, state: "open" }}
			depth={1}
			folded
			meta={
				<>
					<CiStatus status="working" />
					<MetaCount type="unpushed">3</MetaCount>
					<MetaCount type="age">8m</MetaCount>
				</>
			}
			actions={push}
			menu={menu("Branch")}
		/>
		<Stack
			tip="feat: unique cursor names"
			initiallyFolded={folded[1]}
			summary={
				<>
					<MetaCount type="unpushed">1</MetaCount>
					<MetaCount type="age">35m</MetaCount>
				</>
			}
			branches={[
				<BranchItem
					key="names"
					title="feat: unique cursor names"
					pr={{ number: 36, state: "open" }}
					depth={2}
					folded
					meta={
						<>
							<CiStatus status="working" />
							<MetaCount type="unpushed">1</MetaCount>
							<MetaCount type="age">35m</MetaCount>
						</>
					}
					actions={push}
					menu={menu("Branch")}
				/>,
				<BranchItem
					key="api"
					title="cursor-api"
					depth={2}
					folded
					meta={
						<>
							<MetaCount type="commits">5</MetaCount>
							<MetaCount type="age">2d</MetaCount>
						</>
					}
					menu={menu("Branch")}
				/>,
			]}
		/>
	</Repository>
);

/**
 * Stacks of branches, with no divider around them: a stack's branches sit one level under its
 * caption. Folded, the caption names the branch at the top of the stack and sums up the rest; open,
 * it steps back. A click anywhere on it folds the stack.
 */
export const StacksFolded = meta.story({
	parameters: { design: { type: "figma", url: core("3688-8924") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<Stacks folded={[true, false]} />
			</Card>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<Stacks folded={[true, true]} />
			</Card>
		</div>
	),
});

/**
 * A second clone of a repository whose remote is a fork. Its main worktree takes the folder-fork
 * glyph, leads with a Fork badge and goes by the fork's owner, so it doesn't read as a second
 * "Main worktree".
 */
export const Fork = meta.story({
	parameters: { design: { type: "figma", url: core("3357-7989") } },
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Repository last>
				<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
				<WorktreeItem
					main
					name="Main worktree"
					depth={1}
					folded={false}
					meta={
						<>
							<MetaCount type="behind">4</MetaCount>
							<MetaCount type="age">3d</MetaCount>
						</>
					}
					actions={update}
					menu={menu("Worktree")}
				/>
				<UncommittedItem count={19} depth={2} menu={menu("Uncommitted changes")} />
				<BranchItem
					title="ios-push-handoff"
					depth={2}
					folded
					meta={
						<>
							<MetaCount type="unpushed">7</MetaCount>
							<MetaCount type="age">1d</MetaCount>
						</>
					}
					actions={push}
					menu={menu("Branch")}
				/>
				<WorktreeDivider depth={1} />
				<WorktreeItem
					main
					fork
					name="pavellaptev"
					depth={1}
					folded={false}
					meta={
						<>
							<MetaCount type="behind">1</MetaCount>
							<MetaCount type="age">3d</MetaCount>
						</>
					}
					actions={update}
					menu={menu("Worktree")}
				/>
				<UncommittedItem count={22} depth={2} menu={menu("Uncommitted changes")} />
				<BranchItem
					title="review-cards"
					depth={2}
					folded
					meta={
						<>
							<MetaCount type="unpushed">8</MetaCount>
							<MetaCount type="age">12m</MetaCount>
						</>
					}
					actions={push}
					menu={menu("Branch")}
				/>
				<WorktreeDivider depth={1} />
				<WorktreeItem
					name="mesh-archive"
					depth={1}
					folded
					meta={
						<>
							<MetaCount type="unpushed">4</MetaCount>
							<MetaCount type="behind">2</MetaCount>
							<MetaCount type="branch">2</MetaCount>
							<MetaCount type="age">3d</MetaCount>
						</>
					}
					menu={menu("Worktree")}
				/>
			</Repository>
		</Card>
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
			icon={<Icon name="desktop" />}
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

/**
 * Rows that could hold something and hold nothing yet: a branch with no commits, a worktree with
 * no branches and no changes, and a repository whose only worktree is such a one. Each keeps a dot
 * where its chevron would be, so it lines up with the rows that fold and nothing offers to open,
 * and says why first in its state: `empty` for a branch without commits, and for a worktree what it
 * is on, `empty` with nothing applied or `detached`.
 */
export const Empty = meta.story({
	parameters: { design: { type: "figma", url: core("2746-14577") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<Repository last>
					<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
					<UncommittedItem count={10} depth={1} menu={menu("Uncommitted changes")} />
					<BranchItem
						title="redact-remote-creds"
						depth={1}
						folded
						meta={
							<>
								<MetaCount type="commits">11</MetaCount>
								<MetaCount type="age">1d</MetaCount>
							</>
						}
						menu={menu("Branch")}
					/>
					<BranchItem
						title="offline-cache"
						depth={1}
						folded="empty"
						meta={<MetaCount type="empty">empty</MetaCount>}
						menu={menu("Branch")}
					/>
				</Repository>
			</Card>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<Repository last>
					<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
					<WorktreeItem
						main
						name="Main worktree"
						depth={1}
						folded={false}
						meta={<MetaCount type="behind">2</MetaCount>}
						menu={menu("Worktree")}
					/>
					<UncommittedItem count={9} depth={2} menu={menu("Uncommitted changes")} />
					<BranchItem
						title="ios-push-handoff"
						depth={2}
						folded
						meta={<MetaCount type="unpushed">9</MetaCount>}
						menu={menu("Branch")}
					/>
					<WorktreeDivider depth={1} />
					<WorktreeItem
						name="scratch"
						depth={1}
						folded="empty"
						meta={
							<>
								<MetaCount type="head">detached</MetaCount>
								<MetaCount type="age">3d</MetaCount>
							</>
						}
						menu={menu("Worktree")}
					/>
				</Repository>
			</Card>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<Repository>
					<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
					<UncommittedItem count={12} depth={1} menu={menu("Uncommitted changes")} />
					<BranchItem
						title="ios-push-handoff"
						depth={1}
						folded
						meta={<MetaCount type="unpushed">4</MetaCount>}
						menu={menu("Branch")}
					/>
				</Repository>
				<Repository last>
					<RepoItem
						name="skill-soup"
						folded="empty"
						meta={
							<>
								<MetaCount type="head">empty</MetaCount>
								<MetaCount type="behind">2</MetaCount>
								<MetaCount type="age">3d</MetaCount>
							</>
						}
						menu={menu("Repository")}
					/>
				</Repository>
			</Card>
		</div>
	),
});

const PAGE = 12;
const tail = [
	"mesh-archive",
	"review-cards",
	"daemon-idle-cpu",
	"redact-remote-creds",
	"offline-cache",
	"docs-mesh-setup",
	"android-paging",
	"stale-worktree",
	"ios-share-sheet",
	"cursor-api",
	"archive-api",
	"retry-transfers",
	"push-handoff",
	"entry-avatar-pic",
	"hunk-separator",
	"agent-elapsed",
	"skill-soup",
	"ci-state",
	"pr-state-after-merge",
	"resume-on-laptop",
];

/** A repository's twenty branches: the five most recent, then the rest a page at a time. */
const PagedBranches: FC = () => {
	const [pages, setPages] = useState(0);
	const shown = Math.min(tail.length, 5 + pages * PAGE);
	const left = tail.length - shown;
	return (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Repository last>
				<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
				<UncommittedItem count={10} depth={1} menu={menu("Uncommitted changes")} />
				{tail.slice(0, shown).map((title, i) => (
					<BranchItem
						key={title}
						title={title}
						depth={1}
						folded
						meta={
							<>
								<MetaCount type="commits">{(i % 7) + 1}</MetaCount>
								<MetaCount type="age">{i + 1}h</MetaCount>
							</>
						}
						menu={menu("Branch")}
					/>
				))}
				<MoreItem
					depth={1}
					label={left > 0 ? `${Math.min(left, PAGE)} more branches` : "Show fewer branches"}
					expanded={left === 0}
					onClick={() => setPages(left > 0 ? pages + 1 : 0)}
				/>
			</Repository>
		</Card>
	);
};

const moreRepos = [
	"skill-soup",
	"figma-plugin",
	"gitbutler",
	"mesh-lab",
	"dotfiles",
	"blog",
	"photo-sorter",
	"tiny-ci",
	"notes",
	"pnpm-patches",
	"ui-kit",
	"playground",
];

/** A machine's repositories: the recent two open, the other twelve behind a row of their own. */
const PagedRepos: FC = () => {
	const [pages, setPages] = useState(0);
	const shown = Math.min(moreRepos.length, pages * PAGE);
	const left = moreRepos.length - shown;
	return (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Repository>
				<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
				<UncommittedItem count={10} depth={1} menu={menu("Uncommitted changes")} />
				<BranchItem
					title="mesh-archive"
					depth={1}
					folded
					meta={
						<>
							<MetaCount type="unpushed">2</MetaCount>
							<MetaCount type="age">8m</MetaCount>
						</>
					}
					menu={menu("Branch")}
				/>
			</Repository>
			<Repository>
				<RepoItem
					name="berlin-brutalism"
					folded={false}
					meta={
						<>
							<MetaCount type="behind">1</MetaCount>
							<MetaCount type="age">1h</MetaCount>
						</>
					}
					menu={menu("Repository")}
				/>
				<UncommittedItem count={6} depth={1} menu={menu("Uncommitted changes")} />
				<BranchItem
					title="Upgrade to Next 16, React 19 and three r186; reduce GPU usage"
					pr={{ number: 1, state: "merged" }}
					depth={1}
					folded
					meta={
						<>
							<MetaCount type="commits">2</MetaCount>
							<MetaCount type="age">1d</MetaCount>
						</>
					}
					menu={menu("Branch")}
				/>
			</Repository>
			{moreRepos.slice(0, shown).map((name, i) => (
				<Repository key={name}>
					<RepoItem
						name={name}
						folded
						meta={<MetaCount type="age">{i + 2}d</MetaCount>}
						menu={menu("Repository")}
					/>
				</Repository>
			))}
			<Repository last>
				<MoreItem
					label={left > 0 ? `${Math.min(left, PAGE)} more repos` : "Show fewer repos"}
					expanded={left === 0}
					onClick={() => setPages(left > 0 ? pages + 1 : 0)}
				/>
			</Repository>
		</Card>
	);
};

/**
 * A long list ends in a row that pages through the rest: "12 more" opens the next twelve, and once
 * everything shows it reads "Show fewer" and folds the list back. Click it to step through. Under
 * a repository it sits at the branches' depth; a machine's repositories end in one at the top
 * level, ruled off like a repository of its own.
 */
export const Paging = meta.story({
	parameters: { design: { type: "figma", url: core("2585-8164") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<PagedBranches />
			<PagedRepos />
		</div>
	),
});

/** One repository's card when the sidebar is grouped by repository: its machines, folded or open. */
const ByRepositoryTree: FC<{ open: boolean }> = ({ open }) => {
	const age = (value: string) => <MetaCount type="age">{value}</MetaCount>;
	return (
		<Repository last>
			<MachineItem
				name="pavel--macbook-pro-2"
				online
				folded={!open}
				icon={<Icon name="laptop" />}
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
					<UncommittedItem count={12} depth={2} menu={menu("Uncommitted changes")} />
					<BranchItem
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
						title="fix: redact remote credentials"
						pr={{ number: 32, state: "open" }}
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
				icon={<Icon name="desktop" />}
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
 * worktrees shows no age, as each says its own.
 */
export const ByRepository = meta.story({
	parameters: { design: { type: "figma", url: core("2642-10912") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Card label="but-dev" header={repository}>
				<ByRepositoryTree open={false} />
			</Card>
			<Card label="but-dev" header={repository}>
				<ByRepositoryTree open />
			</Card>
		</div>
	),
});

/**
 * A pull request merging from the branch's menu: the row takes the merged look as the merge starts,
 * and `busy` turns a spinner in its glyph's place until it is done. Its checks step aside; what is
 * still to push and its age stay.
 */
export const Merging = meta.story({
	render: () => (
		<BranchItem
			title="fix: redact remote URLs in logs"
			pr={{ number: 32, state: "merged" }}
			busy
			depth={1}
			folded
			meta={
				<>
					<MetaCount type="unpushed">3</MetaCount>
					<MetaCount type="age">8m</MetaCount>
				</>
			}
		/>
	),
});

/**
 * Merged work reads as merged whether or not its pull request is known: a request shows its number,
 * and a branch with none known says Merged beside its name. A worktree whose only branch merged is
 * done with: it shows its age, and Delete under the pointer.
 */
export const Merged = meta.story({
	parameters: { design: { type: "figma", url: core("4017-7390") } },
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 1 }}>
			<WorktreeItem
				name="mesh-archive"
				depth={1}
				folded={false}
				meta={<MetaCount type="age">2h</MetaCount>}
				menu={menu("Worktree")}
			/>
			<BranchItem
				title="feat: archive old worktrees"
				pr={{ number: 31, state: "merged" }}
				depth={2}
				folded
				meta={<MetaCount type="age">3d</MetaCount>}
				menu={menu("Branch")}
			/>
			<BranchItem
				title="tidy-sidebar-rows"
				merged
				depth={2}
				folded
				meta={<MetaCount type="age">5d</MetaCount>}
				menu={menu("Branch")}
			/>
			<WorktreeDivider depth={1} />
			<WorktreeItem
				name="retry-transfers"
				depth={1}
				folded={false}
				meta={<MetaCount type="age">1d</MetaCount>}
				actions={<Button size="small">Delete</Button>}
				menu={menu("Worktree")}
			/>
			<BranchItem
				title="retry-transfers"
				merged
				depth={2}
				folded
				meta={<MetaCount type="age">1d</MetaCount>}
				menu={menu("Branch")}
			/>
		</div>
	),
});

/** A worktree whose branches stopped on conflicts, for {@link Conflicts}. */
const ConflictsTree: FC<{ selected?: boolean }> = ({ selected = false }) => {
	const resolve = (
		<Button size="small" key="resolve">
			Resolve
		</Button>
	);
	return (
		<Repository last>
			<RepoItem name="but-dev" folded={false} menu={menu("Repository")} />
			<WorktreeItem
				main
				name="Main worktree"
				depth={1}
				folded={false}
				meta={<MetaCount type="age">3d</MetaCount>}
				menu={menu("Worktree")}
			/>
			<UncommittedItem count={22} depth={2} menu={menu("Uncommitted changes")} />
			<BranchItem
				title="fix: unique cursor names"
				pr={{ number: 454, state: "open" }}
				depth={2}
				folded
				meta={
					<>
						<MetaCount type="conflicts">2</MetaCount>
						<CiStatus status="working" />
						<MetaCount type="unpushed">5</MetaCount>
					</>
				}
				actions={resolve}
				menu={menu("Branch")}
			/>
			<BranchItem
				title="perf: cheaper status polling"
				pr={{ number: 410, state: "open" }}
				depth={2}
				folded={false}
				meta={
					<>
						<CiStatus status="working" />
						<MetaCount type="unpushed">5</MetaCount>
					</>
				}
				actions={resolve}
				menu={menu("Branch")}
			/>
			<CommitItem
				message="docs: handoff notes"
				conflicted
				depth={3}
				selected={selected}
				menu={menu("Commit")}
			/>
			<CommitItem
				message="feat: 28px hunk separator"
				pushed={false}
				conflicted={selected}
				depth={3}
				menu={menu("Commit")}
			/>
			<CommitItem
				message="chore: separator tokens"
				pushed={false}
				depth={3}
				menu={menu("Commit")}
			/>
		</Repository>
	);
};

/**
 * A branch that stopped on conflicts. Folded, it counts its conflicted commits first in its meta
 * and offers Resolve as its next step; open, each conflicted commit takes the warning in its
 * glyph's place, and the branch drops the count. Selected, the warning keeps its red, inverted.
 */
export const Conflicts = meta.story({
	parameters: { design: { type: "figma", url: core("4827-70089") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Card label="pavel--macbook-pro-2" header={machine}>
				<ConflictsTree />
			</Card>
			<Card label="pavel--macbook-pro-2" header={machine} focused>
				<ConflictsTree selected />
			</Card>
		</div>
	),
});

/**
 * Any row at work, here a worktree updating from its target: `busy` turns a spinner in its glyph's
 * place, as on a merging branch, so what it holds and its menu stay where they were.
 */
export const Updating = meta.story({
	render: () => (
		<WorktreeItem
			name="Main worktree"
			main
			busy
			depth={1}
			folded={false}
			meta={
				<>
					<MetaCount type="behind">3</MetaCount>
					<MetaCount type="age">12m</MetaCount>
				</>
			}
			actions={update}
		/>
	),
});

/**
 * What the target has that the workspace doesn't, over the repository's worktrees: how many
 * commits, with Update beside them. Open, its commits sit under it; a merge opens to the commits
 * it brought in.
 */
export const NewOnTarget = meta.story({
	parameters: { design: { type: "figma", url: core("4275-219045") } },
	render: () => <NewOnTargetTree />,
});

/**
 * A {@link TargetItem} whose commits couldn't load: a NoticeItem says so in their place, with the
 * reason over it and Try again after it.
 */
export const NewOnTargetFailed = meta.story({
	parameters: { design: { type: "figma", url: core("4300-233475") } },
	render: () => (
		<div>
			<TargetItem target="master" count={2} depth={1} folded={false} actions={update} />
			<NoticeItem
				message="Can’t show the commits."
				tooltip="The machine didn’t answer"
				action={{ label: "Try again", onClick: () => {} }}
				depth={2}
			/>
			<WorktreeItem name="Main worktree" main depth={1} folded />
		</div>
	),
});

/** {@link NewOnTarget}'s rows, folding as the sidebar's do. */
const NewOnTargetTree: FC = () => {
	const [open, setOpen] = useState(true);
	const [merge, setMerge] = useState(true);
	return (
		<div>
			<TargetItem
				target="master"
				count={74}
				depth={1}
				folded={!open}
				onToggleFolded={() => setOpen(!open)}
				actions={update}
			/>
			{open && (
				<>
					<CommitItem
						message="chore: separator tokens"
						merge
						depth={2}
						folded={!merge}
						onToggleFolded={() => setMerge(!merge)}
					/>
					{merge && (
						<>
							<CommitItem message="chore: separator tokens" depth={3} />
							<CommitItem message="feature: user authentication" depth={3} />
						</>
					)}
					<CommitItem message="fix: retry stalled transfers" depth={2} />
				</>
			)}
			<WorktreeItem name="Main worktree" main depth={1} folded />
		</div>
	);
};

const DocsNavigation: FC = () => {
	const [folded, setFolded] = useState(false);
	return (
		// In a card, as every sidebar row is: never loose on a panel's ground.
		<nav
			aria-label="Pages"
			style={{
				padding: 8,
				border: "1px solid var(--border-section)",
				borderRadius: "var(--radius-card)",
				backgroundColor: "var(--bg-1)",
			}}
		>
			<GroupLabel
				name="Getting started"
				folded={folded}
				onToggleFolded={() => setFolded(!folded)}
			/>
			{!folded &&
				(
					[
						["Overview", "docs"],
						["Emphasis", "bullseye"],
						["Icons", "file-text"],
						["Buttons", "bullet-list"],
					] as const
				).map(([page, icon]) => (
					<NavItem
						key={page}
						name={page}
						icon={icon}
						depth={1}
						selected={page === "Emphasis"}
						render={
							// oxlint-disable-next-line jsx_a11y/anchor-has-content, jsx_a11y/control-has-associated-label -- The row renders as this link, and NavItem fills it with the name.
							<a
								href={`#${page.toLowerCase()}`}
								aria-current={page === "Emphasis" ? "page" : undefined}
							/>
						}
					/>
				))}
		</nav>
	);
};

/** A docs site's pages: a group label that folds them away, and the pages one level under it, the
 * page on screen selected. */
export const Navigation = meta.story({
	parameters: { design: { type: "figma", url: core("4793-4281") } },
	render: () => <DocsNavigation />,
});
