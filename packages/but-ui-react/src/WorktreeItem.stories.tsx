import { stories } from "../docs/stories.ts";
import { Icon } from "./Icon.tsx";
import { MetaCount } from "./MetaCount.tsx";
import { BranchItem, MachineItem, RepoItem, UncommittedItem, WorktreeItem } from "./SidebarRow.tsx";
import {
	Card,
	Column,
	machine,
	menu,
	push,
	Repository,
	update,
} from "./story-assets/workspaceTree.tsx";

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
 * step. Under the pointer the next step and the menu take their place.
 */
export const Default = meta.story({
	render: (args) => (
		<WorktreeItem
			{...args}
			meta={
				<>
					<MetaCount type="behind">2</MetaCount>
					<MetaCount type="age">5h</MetaCount>
				</>
			}
			actions={update}
			menu={menu("Worktree")}
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
		</Column>
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
