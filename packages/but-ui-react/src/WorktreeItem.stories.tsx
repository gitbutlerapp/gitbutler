import { stories } from "../stories/stories.ts";
import { Icon } from "./Icon.tsx";
import { MetaCount } from "./MetaCount.tsx";
import {
	BranchItem,
	MachineItem,
	RepoItem,
	TerminalItem,
	UncommittedItem,
	WorktreeItem,
} from "./SidebarRow.tsx";
import { Card, Column, machine, push, Repository, update } from "./story-assets/workspaceTree.tsx";
import { Tooltip } from "./Tooltip.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Workspace tree/Machines & worktrees",
	id: "components-sidebarrow-worktree",
	component: WorktreeItem,
	parameters: { width: 316 },
	args: { name: "mesh-archive", depth: 1, folded: true },
});

/**
 * A worktree: how far it is behind its target and its age at the row's end, and Update as its next
 * step. Under the pointer the next step takes their place.
 */
export const Default = meta.story({
	render: (args) => (
		<WorktreeItem {...args} meta={<MetaCount type="age">5h</MetaCount>} actions={update(2)} />
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
		<Column>
			<RepoItem
				name="berlin-brutalism"
				folded={false}
				meta={
					<>
						<MetaCount type="behind">1</MetaCount>
						<MetaCount type="age">2h</MetaCount>
					</>
				}
			/>
			<UncommittedItem count={12} depth={1} />
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
			/>
			<RepoItem name="but-dev" folded={false} />
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
			/>
			<UncommittedItem count={17} depth={2} selected />
			<BranchItem
				title="ios-push-handoff"
				depth={2}
				folded
				meta={<MetaCount type="unpushed">4</MetaCount>}
			/>
		</Column>
	),
});

/**
 * Any row at work, here a worktree updating from its target: `busy` turns a spinner in its glyph's
 * place, as on a merging branch, so what it holds stays where it was.
 */
export const Updating = meta.story({
	render: () => (
		<WorktreeItem
			name="Main worktree"
			main
			busy
			depth={1}
			folded={false}
			meta={<MetaCount type="age">12m</MetaCount>}
			actions={update(3)}
		/>
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
				<RepoItem name="but-dev" folded={false} />
				<WorktreeItem
					main
					name="Main worktree"
					depth={1}
					folded={false}
					meta={<MetaCount type="age">3d</MetaCount>}
					actions={update(4)}
				/>
				<UncommittedItem count={19} depth={2} />
				<BranchItem
					title="ios-push-handoff"
					depth={2}
					folded
					meta={<MetaCount type="age">1d</MetaCount>}
					actions={push(7)}
				/>
				<WorktreeItem
					main
					fork
					name="pavellaptev"
					depth={1}
					folded={false}
					meta={<MetaCount type="age">3d</MetaCount>}
					actions={update(1)}
				/>
				<UncommittedItem count={22} depth={2} />
				<BranchItem
					title="review-cards"
					depth={2}
					folded
					meta={<MetaCount type="age">12m</MetaCount>}
					actions={push(8)}
				/>
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
				/>
			</Repository>
		</Card>
	),
});

/** A machine that is offline: its status goes grey, and says under the pointer when it was last seen. */
export const OfflineMachine = meta.story({
	parameters: { design: { type: "figma", url: core("2579-7658") } },
	render: () => (
		<MachineItem
			name="studio-desktop"
			online={false}
			statusTooltip="Offline · last seen 2 days ago"
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
 * A linked worktree with no branch of its own, its HEAD on the last commit of
 * `sidebar-unread-dot`, a branch in the Main worktree. Its meta says what it's on: `head`, the
 * anchor and the branch's name, which gives way before the worktree's own, and the tooltip says
 * there's no branch. Nothing is listed for the branch under it; the terminal open in it is.
 */
export const NoBranch = meta.story({
	parameters: { design: { type: "figma", url: core("5464-18190") } },
	render: () => (
		<Card label="gitbutler" header={<RepoItem name="gitbutler" folded={false} />}>
			<Repository last>
				<WorktreeItem name="Main worktree" main depth={0} folded />
				<WorktreeItem
					name="gpui-row-buttons"
					depth={0}
					folded={false}
					meta={
						<Tooltip content="No branch: on sidebar-unread-dot's last commit">
							<MetaCount type="head" aria-label="No branch: on sidebar-unread-dot's last commit">
								sidebar-unread-dot
							</MetaCount>
						</Tooltip>
					}
				/>
				<TerminalItem name="claude" depth={1} />
			</Repository>
		</Card>
	),
});
