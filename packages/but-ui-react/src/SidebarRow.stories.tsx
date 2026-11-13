import { stories } from "../docs/stories.ts";
import { Fragment, useState, type FC, type ReactNode } from "react";
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
	WorktreeItem,
} from "./SidebarRow.tsx";
import {
	Card,
	machine,
	menu,
	push,
	Repository,
	update,
	viewPr,
} from "./story-assets/workspaceTree.tsx";
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
	/** A git object, a stack, a branch or a commit: it meets its parent's guide with a stub. */
	chained?: boolean;
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
	const guides = treeGuides(shown.map(({ node, depth }) => ({ depth, chained: node.chained })));
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
		chained: true,
		row: (place) => (
			<CommitItem {...place} message={message} pushed={index >= unpushed} menu={menu("Commit")} />
		),
	}));

/** The same machine as the tree above, as data, so every row with something under it folds. */
const repositories: ReadonlyArray<TreeNode> = [
	{
		id: "but-dev",
		row: (place, fold) => (
			<RepoItem {...place} {...fold} name="but-dev" menu={menu("Repository")} />
		),
		children: [
			{
				id: "main",
				row: (place, fold) => (
					<WorktreeItem
						{...place}
						{...fold}
						main
						name="Main worktree"
						meta={
							<>
								<MetaCount type="behind">4</MetaCount>
								{since("1d")}
							</>
						}
						actions={update}
						menu={menu("Worktree")}
					/>
				),
				children: [
					{
						id: "main-uncommitted",
						row: (place) => (
							<UncommittedItem {...place} count={19} menu={menu("Uncommitted changes")} />
						),
					},
					{
						id: "ios-push-handoff",
						chained: true,
						row: (place, fold) => (
							<BranchItem
								{...place}
								{...fold}
								title="ios-push-handoff"
								meta={
									<>
										<MetaCount type="unpushed">7</MetaCount>
										{since("1d")}
									</>
								}
								actions={push}
								menu={menu("Branch")}
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
						chained: true,
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
								menu={menu("Stack")}
							/>
						),
						children: [
							{
								id: "pr-377",
								chained: true,
								row: (place, fold) => (
									<BranchItem
										{...place}
										{...fold}
										title="fix: retry stalled transfers"
										pr={{ number: 377, state: "open" }}
										meta={
											<>
												<CiStatus status="working" />
												<MetaCount type="unpushed">4</MetaCount>
												{since("8m")}
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
								children: commits(
									"pr-377",
									["fix: back off between retries", "fix: retry stalled transfers"],
									2,
								),
							},
							{
								id: "pr-280",
								chained: true,
								row: (place, fold) => (
									<BranchItem
										{...place}
										{...fold}
										title="feat: resume work on another machine"
										pr={{ number: 280, state: "open" }}
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
						chained: true,
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
								menu={menu("Branch")}
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
						meta={
							<>
								<MetaCount type="behind">2</MetaCount>
								{since("5h")}
							</>
						}
						actions={update}
						menu={menu("Worktree")}
					/>
				),
				children: [
					{
						id: "mesh-uncommitted",
						row: (place) => (
							<UncommittedItem {...place} count={2} menu={menu("Uncommitted changes")} />
						),
					},
					{
						id: "resume-on-laptop",
						chained: true,
						row: (place, fold) => (
							<BranchItem
								{...place}
								{...fold}
								title="resume-on-laptop"
								meta={
									<>
										<MetaCount type="unpushed">9</MetaCount>
										{since("5h")}
									</>
								}
								actions={push}
								menu={menu("Branch")}
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
						chained: true,
						row: (place) => (
							<BranchItem
								{...place}
								title="offline-cache"
								meta={<MetaCount type="empty">empty</MetaCount>}
								menu={menu("Branch")}
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
				meta={
					<>
						<MetaCount type="behind">8</MetaCount>
						{since("2h")}
					</>
				}
				actions={update}
				menu={menu("Repository")}
			/>
		),
		children: [
			{
				id: "daemon-idle-cpu",
				chained: true,
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
						menu={menu("Branch")}
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
					menu={menu("Machine")}
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
 * Every row that holds others folds, its chevron in its glyph's place under the pointer and on
 * keyboard focus. The last row under each parent ends its guide with a tail into its glyph;
 * stacks, branches and commits meet theirs with a stub (`guides`), which the host works out from the tree.
 */
export const Folding = meta.story({
	render: () => <FoldingCard />,
});

/**
 * A selected row: the solid fill while the list holds the selection focus, inverting what it
 * shows, and the quiet one in a list that has lost it. A selected row shows its menu, as the
 * pointer would.
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
