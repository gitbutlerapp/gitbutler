import { stories } from "../docs/stories.ts";
import { useState, type FC, type ReactNode } from "react";
import { Button } from "./Button.tsx";
import { CiStatus, MetaCount } from "./MetaCount.tsx";
import {
	BranchItem,
	CommitItem,
	RepoItem,
	StackCaption,
	UncommittedItem,
	WorktreeItem,
} from "./SidebarRow.tsx";
import { Card, Column, machine, menu, push, Repository } from "./story-assets/workspaceTree.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Workspace tree/Branches",
	id: "components-sidebarrow-branch",
	component: BranchItem,
	parameters: { width: 316 },
	args: { title: "ios-push-handoff", depth: 1, folded: true },
});

/**
 * A branch with no pull request: its name as `title`, and what is still to push at the row's end.
 * Under the pointer the next step and the menu take their place.
 */
export const Default = meta.story({
	render: (args) => (
		<BranchItem
			{...args}
			meta={<MetaCount type="unpushed">7</MetaCount>}
			actions={push}
			menu={menu("Branch")}
		/>
	),
});

/** A stack's caption over its branches, which it hides while folded and sums up instead. */
const stack = ({
	tip,
	branches,
	summary,
	folded,
	onToggleFolded,
}: {
	tip: string;
	branches: Array<ReactNode>;
	summary: ReactNode;
	folded: boolean;
	onToggleFolded: () => void;
}) => (
	<>
		<StackCaption
			count={branches.length}
			tip={tip}
			depth={1}
			folded={folded}
			onToggleFolded={onToggleFolded}
			meta={folded && summary}
			menu={menu("Stack")}
		/>
		{!folded && branches}
	</>
);

/** One repository's branches: two stacks around a branch of its own. */
const Stacks: FC<{ folded: [boolean, boolean] }> = ({ folded: initiallyFolded }) => {
	const [folded, setFolded] = useState(initiallyFolded);
	return (
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
			{stack({
				tip: "feat: archive UI",
				folded: folded[0],
				onToggleFolded: () => setFolded([!folded[0], folded[1]]),
				summary: (
					<>
						<MetaCount type="unpushed">5</MetaCount>
						<MetaCount type="age">12m</MetaCount>
					</>
				),
				branches: [
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
				],
			})}
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
			{stack({
				tip: "feat: unique cursor names",
				folded: folded[1],
				onToggleFolded: () => setFolded([folded[0], !folded[1]]),
				summary: (
					<>
						<MetaCount type="unpushed">1</MetaCount>
						<MetaCount type="age">35m</MetaCount>
					</>
				),
				branches: [
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
				],
			})}
		</Repository>
	);
};

/**
 * Stacks of branches, with no divider around them: a stack's branches sit one level under its
 * caption. Folded, the caption names the branch at the top of the stack and sums up the rest; open,
 * it steps back. A click anywhere on it folds the stack.
 */
export const StacksFolded = meta.story({
	parameters: { design: { type: "figma", url: core("4664-11727") } },
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
 * Rows that could hold something and hold nothing yet: a branch with no commits, a worktree with
 * no branches and no changes, and a repository whose only worktree is such a one. Each has no
 * chevron, so nothing offers to open, and says why first in its state: `empty` for a branch without commits, and for a worktree what it
 * is on, `empty` with nothing applied or `detached`.
 */
export const Empty = meta.story({
	parameters: { design: { type: "figma", url: core("4664-11725") } },
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
					<WorktreeItem
						name="scratch"
						depth={1}
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
	parameters: { design: { type: "figma", url: core("2572-3458") } },
	render: () => (
		<Column>
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
		</Column>
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
