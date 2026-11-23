import { stories } from "../stories/stories.ts";
import { Fragment, useState, type FC, type ReactNode } from "react";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { ContextMenu, MenuItem, MenuSection } from "./Menu.tsx";
import { MetaCount, MetaStatus } from "./MetaCount.tsx";
import {
	BranchItem,
	CommitItem,
	LoadingItem,
	MachineItem,
	MoreItem,
	NoticeItem,
	RepoItem,
	StackCaption,
	UncommittedItem,
	UpstreamItem,
	WorktreeItem,
} from "./SidebarRow.tsx";
import { Card, machine, push, Repository, update } from "./story-assets/workspaceTree.tsx";
import { treeGuides, type Guides } from "./treeGuides.ts";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Workspace tree/Tree",
	id: "components-sidebarrow",
	component: BranchItem,
	parameters: { design: { type: "figma", url: core("2572-3458") }, width: 316 },
	args: {
		title: "fix: retry stalled transfers",
		pr: { number: 377, state: "open" },
		merged: false,
		depth: 1,
		folded: true,
		selected: false,
	},
});

/**
 * A branch with an open pull request: its checks and what is still to push at the row's end. Under
 * the pointer the next step takes their place. Its menu opens with a right-click, or Shift-F10 on
 * the focused row: the row is a `ContextMenu`'s `trigger`.
 */
export const Default = meta.story({
	args: { title: "fix: retry stalled transfers" },
	render: (args) => (
		<ContextMenu
			aria-label="Branch actions"
			trigger={
				<BranchItem
					{...args}
					meta={<MetaStatus variant="working">CI</MetaStatus>}
					actions={<>{push(4)}</>}
				/>
			}
		>
			<MenuSection>
				<MenuItem>Push</MenuItem>
				<MenuItem>Open pull request</MenuItem>
				<MenuItem>Copy branch name</MenuItem>
			</MenuSection>
			<MenuSection>
				<MenuItem>Delete branch</MenuItem>
			</MenuSection>
		</ContextMenu>
	),
});

/** A machine: two repositories, one with two worktrees and a stack. */
const Tree: FC<{ selected?: string }> = ({ selected }) => {
	const age = (value: string) => <MetaCount type="age">{value}</MetaCount>;
	const [stackFolded, setStackFolded] = useState(false);
	return (
		<>
			<Repository>
				<RepoItem name="but-dev" folded={false} />
				<WorktreeItem
					main
					name="Main worktree"
					depth={1}
					folded={false}
					meta={<>{age("1d")}</>}
					actions={update(4)}
				/>
				<UncommittedItem count={19} depth={2} selected={selected === "uncommitted"} />
				<BranchItem
					title="ios-push-handoff"
					depth={2}
					folded
					selected={selected === "ios-push-handoff"}
					actions={push(7)}
				/>
				<StackCaption
					count={2}
					tip="fix: retry stalled transfers"
					depth={2}
					folded={stackFolded}
					onToggleFolded={() => setStackFolded(!stackFolded)}
					meta={stackFolded && <MetaCount type="age">8m</MetaCount>}
					actions={stackFolded && push(11)}
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
									<MetaStatus variant="working">CI</MetaStatus>
									{age("8m")}
								</>
							}
							actions={<>{push(4)}</>}
						/>
						<BranchItem
							title="feat: resume work on another machine"
							pr={{ number: 280, state: "open" }}
							depth={3}
							folded={false}
							meta={<MetaStatus variant="working">CI</MetaStatus>}
							actions={<>{push(7)}</>}
						/>
						<CommitItem message="chore: separator tokens" pushed={false} depth={4} />
						<CommitItem
							message="feat: 28px hunk separator"
							depth={4}
							selected={selected === "commit"}
						/>
						<CommitItem message="feat: elapsed time for agents" depth={4} />
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
				/>
				<MoreItem label="3 more" depth={2} />
				<WorktreeItem
					name="mesh-archive"
					depth={1}
					folded={false}
					meta={<>{age("5h")}</>}
					actions={update(2)}
				/>
				<UncommittedItem count={2} depth={2} />
				<BranchItem title="resume-on-laptop" depth={2} folded actions={push(9)} />
				<BranchItem
					title="offline-cache"
					depth={2}
					folded="empty"
					meta={<MetaCount type="empty">empty</MetaCount>}
				/>
			</Repository>
			<Repository last>
				<RepoItem
					name="berlin-brutalism"
					folded={false}
					meta={<>{age("2h")}</>}
					actions={update(8)}
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
				/>
			</Repository>
		</>
	);
};

/**
 * Grouped by machine: one line a row, its state at the end, and its next step after it, always:
 * Push and Update in place of the count they act on. The time is optional: a branch as fresh as its
 * worktree leaves it out.
 */
export const ByMachine = meta.story({
	parameters: { design: { type: "figma", url: core("2594-15724") } },
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Tree />
		</Card>
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
			/>
			{open && (
				<>
					<WorktreeItem
						main
						name="Main worktree"
						depth={1}
						folded={false}
						meta={<>{age("8m")}</>}
						actions={update(3)}
					/>
					<UncommittedItem count={12} depth={2} />
					<BranchItem
						title="mesh-archive"
						depth={2}
						folded
						meta={<MetaCount type="agent-working">12m</MetaCount>}
						actions={push(4)}
					/>
					<BranchItem
						title="fix: redact remote credentials"
						pr={{ number: 32, state: "open" }}
						depth={2}
						folded
						meta={
							<>
								<MetaStatus variant="working">CI</MetaStatus>
								{age("3d")}
							</>
						}
						actions={<>{push(4)}</>}
					/>
					<WorktreeItem
						name="review-ios"
						depth={1}
						folded
						meta={<MetaCount type="age">1d</MetaCount>}
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
							<MetaCount type="age">2d</MetaCount>
						</>
					)
				}
				actions={update(3)}
			/>
			{open && (
				<>
					<BranchItem title="resume-on-laptop" depth={1} folded actions={push(4)} />
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
			/>
		</Repository>
	);
};

const repository = <RepoItem name="but-dev" folded={false} />;

/**
 * Grouped by repository: the repository heads the card and its machines are rows inside it, each
 * over its own worktrees, or straight over the branches of its only one. Folded, a machine sums up
 * its worktrees and its age, and a cloud machine shows its agents' time. Open, a row over several
 * worktrees shows no age, as each says its own.
 */
export const ByRepository = meta.story({
	parameters: { design: { type: "figma", url: core("4664-11724") } },
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

/** Where a row sits: its level, and its guides, worked out from where it is in the tree. */
type Place = { depth: number; guides: Guides };
/** A row that opens: whether it is folded, and how to fold it. */
type Fold = { folded?: boolean; onToggleFolded?: () => void };

/** A row of the tree and the rows under it, as a host holds them before laying them out. */
type TreeNode = {
	id: string;
	row: (place: Place, fold: Fold) => ReactNode;
	children?: ReadonlyArray<TreeNode>;
};

/** Lays the tree out as the flat list of rows it is drawn as, skipping what is folded, and gives
 * each row its guides from that list with `treeGuides`. */
const layOut = (
	nodes: ReadonlyArray<TreeNode>,
	folded: ReadonlySet<string>,
	toggle: (id: string) => void,
): Array<ReactNode> => {
	const shown: Array<{ node: TreeNode; depth: number }> = [];
	const visit = (list: ReadonlyArray<TreeNode>, depth: number) => {
		for (const node of list) {
			shown.push({ node, depth });
			if (node.children !== undefined && !folded.has(node.id)) visit(node.children, depth + 1);
		}
	};
	visit(nodes, 0);
	const guides = treeGuides(shown.map(({ depth }) => ({ depth })));
	return shown.map(({ node, depth }, index) => (
		<Fragment key={node.id}>
			{node.row(
				{ depth, guides: guides[index] ?? [] },
				node.children === undefined
					? {}
					: { folded: folded.has(node.id), onToggleFolded: () => toggle(node.id) },
			)}
		</Fragment>
	));
};

const since = (value: string) => <MetaCount type="age">{value}</MetaCount>;

const commits = (id: string, messages: ReadonlyArray<string>, unpushed = 0): Array<TreeNode> =>
	messages.map((message, index) => ({
		id: `${id}-${index}`,
		row: (place) => <CommitItem {...place} message={message} pushed={index >= unpushed} />,
	}));

/** The same machine as the tree above, as data, so every row with something under it folds. */
const repositories: ReadonlyArray<TreeNode> = [
	{
		id: "but-dev",
		row: (place, fold) => <RepoItem {...place} {...fold} name="but-dev" />,
		children: [
			{
				id: "main",
				row: (place, fold) => (
					<WorktreeItem
						{...place}
						{...fold}
						main
						name="Main worktree"
						meta={<>{since("1d")}</>}
						actions={update(4)}
					/>
				),
				children: [
					{
						id: "main-uncommitted",
						row: (place) => <UncommittedItem {...place} count={19} />,
					},
					{
						id: "ios-push-handoff",
						row: (place, fold) => (
							<BranchItem
								{...place}
								{...fold}
								title="ios-push-handoff"
								meta={<>{since("1d")}</>}
								actions={push(7)}
							/>
						),
						children: commits(
							"ios-push-handoff",
							["feat: hand off pushes to the phone", "chore: push entitlements"],
							2,
						),
					},
					{
						id: "stack",
						row: (place, fold) => (
							<StackCaption
								{...place}
								{...fold}
								count={2}
								tip="fix: retry stalled transfers"
								meta={
									fold.folded && (
										<>
											<MetaCount type="unpushed">11</MetaCount>
											{since("8m")}
										</>
									)
								}
							/>
						),
						children: [
							{
								id: "pr-377",
								row: (place, fold) => (
									<BranchItem
										{...place}
										{...fold}
										title="fix: retry stalled transfers"
										pr={{ number: 377, state: "open" }}
										meta={
											<>
												<MetaStatus variant="working">CI</MetaStatus>
												{since("8m")}
											</>
										}
										actions={<>{push(4)}</>}
									/>
								),
								children: commits(
									"pr-377",
									["fix: back off between retries", "fix: retry stalled transfers"],
									2,
								),
							},
							{
								id: "pr-280",
								row: (place, fold) => (
									<BranchItem
										{...place}
										{...fold}
										title="feat: resume work on another machine"
										pr={{ number: 280, state: "open" }}
										meta={<MetaStatus variant="working">CI</MetaStatus>}
										actions={<>{push(7)}</>}
									/>
								),
								children: commits(
									"pr-280",
									[
										"chore: separator tokens",
										"feat: 28px hunk separator",
										"feat: elapsed time for agents",
									],
									1,
								),
							},
						],
					},
					{
						id: "pr-292",
						row: (place, fold) => (
							<BranchItem
								{...place}
								{...fold}
								title="chore: bump dependencies"
								pr={{ number: 292, state: "merged" }}
								meta={
									<>
										<MetaCount type="commits">12</MetaCount>
										{since("8m")}
									</>
								}
							/>
						),
						children: commits("pr-292", ["chore: bump vite", "chore: bump react"]),
					},
					{ id: "main-more", row: (place) => <MoreItem {...place} label="3 more" /> },
				],
			},
			{
				id: "mesh-archive",
				row: (place, fold) => (
					<WorktreeItem
						{...place}
						{...fold}
						name="mesh-archive"
						meta={<>{since("5h")}</>}
						actions={update(2)}
					/>
				),
				children: [
					{
						id: "mesh-uncommitted",
						row: (place) => <UncommittedItem {...place} count={2} />,
					},
					{
						id: "resume-on-laptop",
						row: (place, fold) => (
							<BranchItem
								{...place}
								{...fold}
								title="resume-on-laptop"
								meta={<>{since("5h")}</>}
								actions={push(9)}
							/>
						),
						children: commits(
							"resume-on-laptop",
							["feat: pick up where the laptop left off", "feat: sync open files"],
							2,
						),
					},
					{
						id: "offline-cache",
						row: (place) => (
							<BranchItem
								{...place}
								title="offline-cache"
								folded="empty"
								meta={<MetaCount type="empty">empty</MetaCount>}
							/>
						),
					},
				],
			},
		],
	},
	{
		id: "berlin-brutalism",
		row: (place, fold) => (
			<RepoItem
				{...place}
				{...fold}
				name="berlin-brutalism"
				meta={<>{since("2h")}</>}
				actions={update(8)}
			/>
		),
		children: [
			{
				id: "daemon-idle-cpu",
				row: (place, fold) => (
					<BranchItem
						{...place}
						{...fold}
						title="daemon-idle-cpu"
						meta={
							<>
								<MetaCount type="commits">27</MetaCount>
								{since("5h")}
							</>
						}
						actions={<Button size="small">Create PR</Button>}
					/>
				),
				children: commits("daemon-idle-cpu", [
					"perf: sleep while the daemon is idle",
					"perf: batch file events",
				]),
			},
		],
	},
];

/** The machine's card with every row that holds others folding, as a host draws it. */
const FoldingCard: FC = () => {
	const [folded, setFolded] = useState<ReadonlySet<string>>(
		() => new Set(["ios-push-handoff", "pr-377", "pr-292", "resume-on-laptop", "daemon-idle-cpu"]),
	);
	const [machineFolded, setMachineFolded] = useState(false);
	const toggle = (id: string) =>
		setFolded((current) => {
			const next = new Set(current);
			if (!next.delete(id)) next.add(id);
			return next;
		});
	return (
		<Card
			label="pavel--macbook-pro-2"
			header={
				<MachineItem
					name="pavel--macbook-pro-2"
					online
					folded={machineFolded}
					onToggleFolded={() => setMachineFolded(!machineFolded)}
					icon={<Icon name="laptop" />}
				/>
			}
		>
			{!machineFolded &&
				repositories.map((repository, index) => (
					<Repository key={repository.id} last={index === repositories.length - 1}>
						{layOut([repository], folded, toggle)}
					</Repository>
				))}
		</Card>
	);
};

/**
 * Every row that holds others folds, its chevron in a step of its own before its glyph; a branch
 * with no commits yet keeps a dot there. The last row under each parent ends its guide with a tail
 * into its glyph (`guides`), which the host works out from the tree.
 */
export const Folding = meta.story({
	render: () => <FoldingCard />,
});

/**
 * A selected row: the solid fill while the list holds the selection focus, inverting what it
 * shows, and the quiet one in a list that has lost it. A selected row keeps its state at its
 * end.
 */
export const Selection = meta.story({
	parameters: { design: { type: "figma", url: core("2594-15770") } },
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
				<RepoItem name="but-dev" folded={false} />
				<UncommittedItem count={10} depth={1} />
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
				<RepoItem name="but-dev" folded={false} />
				<UncommittedItem count={10} depth={1} />
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
				/>
				<UncommittedItem count={6} depth={1} />
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
				/>
			</Repository>
			{moreRepos.slice(0, shown).map((name, i) => (
				<Repository key={name}>
					<RepoItem name={name} folded meta={<MetaCount type="age">{i + 2}d</MetaCount>} />
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

/** A branch in {@link Unread}: its commits, newest first, and which of them are new to the reader. */
type UnreadBranch = {
	id: string;
	title: string;
	pr?: { number: number; state: "open" | "merged" };
	pushed: boolean;
	commits: ReadonlyArray<{ message: string; unread?: boolean }>;
	push?: number;
};

const unreadBranches: ReadonlyArray<UnreadBranch> = [
	{
		id: "review-cards",
		title: "review-cards",
		pushed: false,
		push: 3,
		commits: [
			{ message: "feat: review card layout", unread: true },
			{ message: "fix: card hover ground", unread: true },
			{ message: "chore: card tokens" },
		],
	},
	{
		id: "454",
		title: "fix: unique cursor names",
		pr: { number: 454, state: "open" },
		pushed: false,
		push: 2,
		commits: [{ message: "fix: unique cursor names" }, { message: "test: cursor names" }],
	},
	{
		id: "cursor-api",
		title: "cursor-api",
		pushed: true,
		commits: [
			{ message: "feat: cursor endpoint", unread: true },
			{ message: "feat: cursor paging" },
			{ message: "docs: cursor api" },
		],
	},
	{
		id: "132",
		title: "feat: archive old worktrees",
		pr: { number: 132, state: "merged" },
		pushed: true,
		commits: [{ message: "feat: archive old worktrees" }, { message: "test: archive" }],
	},
];

/**
 * {@link Unread}'s rows. A folded branch carries the dot for any commit inside it that's new;
 * open, the dot moves to those commits. Selecting a commit reads it, and once none inside is new,
 * the branch has no dot, folded or not.
 */
const UnreadTree: FC = () => {
	const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
	const [read, setRead] = useState<ReadonlySet<string>>(new Set());
	const [selected, setSelected] = useState<string | undefined>();
	const isNew = (branch: UnreadBranch, message: string) =>
		branch.commits.some((c) => c.message === message && c.unread) && !read.has(message);
	const anyNew = (branch: UnreadBranch) => branch.commits.some((c) => isNew(branch, c.message));
	const toggle = (id: string) => () => {
		const next = new Set(open);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		setOpen(next);
	};
	return (
		<Repository last>
			<RepoItem name="but-dev" folded={false} />
			<UncommittedItem count={10} depth={1} />
			{unreadBranches.map((branch) => {
				const isOpen = open.has(branch.id);
				return (
					<Fragment key={branch.id}>
						<BranchItem
							title={branch.title}
							pr={branch.pr}
							depth={1}
							folded={!isOpen}
							onToggleFolded={toggle(branch.id)}
							unread={!isOpen && anyNew(branch)}
							meta={
								branch.pushed &&
								!isOpen && <MetaCount type="commits">{branch.commits.length}</MetaCount>
							}
							actions={branch.push !== undefined && push(branch.push)}
						/>
						{isOpen &&
							branch.commits.map((commit) => (
								<CommitItem
									key={commit.message}
									message={commit.message}
									pushed={branch.pushed}
									depth={2}
									unread={isNew(branch, commit.message)}
									selected={selected === commit.message}
									onClick={() => {
										setSelected(commit.message);
										setRead(new Set(read).add(commit.message));
									}}
								/>
							))}
					</Fragment>
				);
			})}
		</Repository>
	);
};

/**
 * Rows that changed since they were last opened take a dot (`unread`) in their start padding,
 * before the guides, where it moves nothing, so the counts line up whether a row has one or not.
 * A folded branch carries the dot for its new commits; open it and the dot moves to them. Select a
 * commit to read it: once a branch has none new, its dot goes.
 */
export const Unread = meta.story({
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<UnreadTree />
		</Card>
	),
});

const pavel = { seed: "pavel@gitbutler.com" };
const kiril = { seed: "kiril@gitbutler.com" };

/** {@link Upstream}'s rows, the heading and a merge folding as the sidebar's do. */
const UpstreamTree: FC = () => {
	const [branch, setBranch] = useState(true);
	const [open, setOpen] = useState(true);
	const [merge, setMerge] = useState(true);
	return (
		<Repository last>
			<RepoItem name="but-dev" folded={false} />
			<BranchItem
				title="perf: cheaper status polling"
				depth={1}
				folded={!branch}
				onToggleFolded={() => setBranch(!branch)}
				meta={<MetaStatus variant="working">CI</MetaStatus>}
				authors={[pavel, kiril]}
				actions={!branch && update(3, "origin")}
			/>
			{branch && (
				<>
					<UpstreamItem
						remote="origin"
						count={3}
						authors={[pavel, kiril]}
						tooltip="3 new commits on origin/perf-cheaper-status-polling, from Pavel Laptev and Kiril Videlov"
						depth={2}
						folded={!open}
						onToggleFolded={() => setOpen(!open)}
						actions={update(3, "origin")}
					/>
					{open && (
						<>
							<CommitItem
								message="Merge #12: retry on timeout"
								upstream
								merge
								authors={[kiril]}
								tooltip="Kiril Videlov · 1h ago"
								depth={3}
								folded={!merge}
								onToggleFolded={() => setMerge(!merge)}
							/>
							{merge && (
								<CommitItem
									message="fix: retry on timeout"
									upstream
									author={kiril}
									tooltip="Kiril Videlov · 1h ago"
									depth={4}
								/>
							)}
							<CommitItem
								message="test: cover a stalled transfer"
								upstream
								author={pavel}
								tooltip="Pavel Laptev · 2h ago"
								depth={3}
							/>
						</>
					)}
					<CommitItem message="docs: handoff notes" depth={2} />
					<CommitItem message="feat: 28px hunk separator" pushed={false} depth={2} />
					<CommitItem message="chore: separator tokens" pushed={false} depth={2} />
				</>
			)}
		</Repository>
	);
};

/**
 * What a branch's remote has that the branch doesn't, as GPUI shows it: a "New on origin" heading
 * one level under the branch, over its own commits, with who the commits come from and Update in
 * place of the count. Open, the commits sit under it, `upstream` and amber, each with its author's
 * picture at its end, a merge folding to the commits it brought in. Fold the heading and its
 * pictures say who; fold the branch and it says both, the pictures and Update in place of its ↓.
 */
export const Upstream = meta.story({
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<UpstreamTree />
		</Card>
	),
});

/**
 * A remote whose commits couldn't load: a NoticeItem says so in their place, with the reason over
 * it and Try again after it.
 */
export const UpstreamFailed = meta.story({
	render: () => (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Repository last>
				<RepoItem name="but-dev" folded={false} />
				<BranchItem title="perf: cheaper status polling" depth={1} folded={false} />
				<UpstreamItem
					remote="origin"
					count={2}
					tooltip="2 new commits on origin/perf-cheaper-status-polling"
					depth={2}
					folded={false}
					actions={update(2, "origin")}
				/>
				<NoticeItem
					message="Can’t show the commits."
					tooltip="The machine didn’t answer"
					action={{ label: "Try again", onClick: () => {} }}
					depth={3}
				/>
				<CommitItem message="feat: 28px hunk separator" pushed={false} depth={2} />
			</Repository>
		</Card>
	),
});

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

const inactive = [
	{ name: "dazzling-sinoussi-031b8a", age: "6d" },
	{ name: "quirky-hopper-7f2e10", age: "9d" },
];

/** A repository whose linked worktrees nobody touched lately, folded or not by `open`. */
const InactiveWorktreesCard: FC<{ open: boolean }> = ({ open: initial }) => {
	const [open, setOpen] = useState(initial);
	return (
		<Card label="pavel--macbook-pro-2" header={machine}>
			<Repository last>
				<RepoItem name="but-dev" folded={false} />
				<WorktreeItem main name="Main worktree" depth={1} folded meta={since("1d")} />
				<WorktreeItem name="mesh-archive" depth={1} folded meta={since("5h")} />
				{open &&
					inactive.map((worktree) => (
						<WorktreeItem
							key={worktree.name}
							name={worktree.name}
							depth={1}
							folded
							meta={since(worktree.age)}
						/>
					))}
				<MoreItem
					depth={1}
					label={open ? "Hide inactive worktrees" : `${inactive.length} inactive worktrees`}
					expanded={open}
					onClick={() => setOpen(!open)}
				/>
			</Repository>
		</Card>
	);
};

/**
 * Linked worktrees with nothing uncommitted that nobody touched lately fold away behind a row at
 * the worktrees' depth, after the active ones: "2 inactive worktrees" shows them, and "Hide
 * inactive worktrees" folds them back. Click it to switch.
 */
export const InactiveWorktrees = meta.story({
	parameters: { design: { type: "figma", url: core("2585-8164") } },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<InactiveWorktreesCard open={false} />
			<InactiveWorktreesCard open />
		</div>
	),
});

/**
 * A repository's card while its rows load: the header as it will be, over rows of bones, each a
 * glyph and a name, so the card holds its place and nothing moves as the rows land.
 */
export const Loading = meta.story({
	parameters: { design: { type: "figma", url: core("5500-237409") } },
	render: () => (
		<Card label="but-dev" header={<RepoItem name="but-dev" folded={false} />}>
			<Repository last>
				<div aria-busy style={{ display: "flex", flexDirection: "column", gap: 1 }}>
					{[156, 76, 199, 116].map((width) => (
						<LoadingItem key={width} width={width} />
					))}
				</div>
			</Repository>
		</Card>
	),
});
