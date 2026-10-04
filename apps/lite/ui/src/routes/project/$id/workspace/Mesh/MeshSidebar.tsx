import {
	branchAddress,
	commitAddress,
	fileAddress,
	worktreeChangesFileParent,
} from "#ui/addresses.ts";
import { Match } from "effect";
import { commitBody, commitTitle } from "#ui/commit.ts";
import { encodeCursorParam, type UrlQueryParams } from "#ui/cursor-url.ts";
import { type FocusScope, useAddressSpaceHotkeys, useAutofocusScope } from "#ui/focus-scopes.ts";
import { sidebarHotkeys } from "#ui/hotkeys.ts";
import { interfaceSlice, type MeshGrouping, type MeshOverview } from "#ui/interface/state.ts";
import { useHostedBranchDismiss, useHostedBranchPull } from "#ui/api/mutations.ts";
import { useHostedSync } from "#ui/HostedSync.tsx";
import {
	type NativeMenuItem,
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
} from "#ui/native-menu.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { NotificationBell } from "#ui/review-inbox-bell.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { setActiveList, setCursor, useActiveList } from "#ui/use-cursor.ts";
import { buildIndexByKey } from "#ui/workspace/address-space.ts";
import { Toggle, ToggleGroup, Toolbar, Tooltip } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import { Badge } from "@gitbutler/ui-react/Badge.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { FileListItem } from "@gitbutler/ui-react/FileList.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { RelativeTime } from "@gitbutler/ui-react/RelativeTime.tsx";
import { ScrollArea } from "@gitbutler/ui-react/ScrollArea.tsx";
import { ToggleGroupStyles, ToggleStyles } from "@gitbutler/ui-react/ToggleGroup.tsx";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useNavigate } from "@tanstack/react-router";
import { type FC, type ReactNode, useRef, useState } from "react";
import { FileRowTooltipRoot, type FileRowTooltipPayload } from "../FileRowTooltip.tsx";
import { getRowButtonClassName, rowPointerProps } from "../Row-utils.ts";
import { SidebarHeader } from "../SidebarHeader.tsx";
import { useFetchFromRemotes } from "../useFetchFromRemotes.ts";
import { buildMeshRows, type MeshRow } from "./mesh-rows.ts";
import { type MeshBranch, type MeshCheckout, useMeshTree } from "./useMeshTree.ts";
import { MeshWatchers } from "./MeshWatchers.tsx";
import styles from "./MeshSidebar.module.css";

/**
 * The sidebar as a mesh, like but.dev's navigator: every local project and every machine that
 * published one, grouped by machine or by repo.
 */
export const MeshSidebar: FC<{ project: ProjectForFrontend; projectId: string }> = ({
	project,
	projectId,
}) => {
	const dispatch = useAppDispatch();
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const fetchFromRemotes = useFetchFromRemotes(projectId);
	const grouping = useAppSelector(interfaceSlice.selectors.selectMeshGrouping);

	return (
		<div className={styles.container}>
			<div className={styles.top}>
				<SidebarHeader
					bell={<NotificationBell projectId={projectId} />}
					project={project}
					isFetchPending={fetchFromRemotes.isPending}
					canOpenOperationsLog={noOperationPending}
					onOpenOperationsLog={() =>
						dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "OperationsLogPicker" } }))
					}
					canOpenSettings={noOperationPending}
					onOpenSettings={() =>
						dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "Settings" } }))
					}
				/>
				<ToggleGroup
					render={<ToggleGroupStyles />}
					aria-label="Group by"
					value={[grouping]}
					onValueChange={(value: Array<MeshGrouping>) => {
						const head = value[0];
						if (head !== undefined)
							dispatch(interfaceSlice.actions.setMeshGrouping({ grouping: head }));
					}}
				>
					<Toggle
						render={<ToggleStyles />}
						value={"machines" satisfies MeshGrouping}
						aria-label="Machines"
					>
						<Icon name="workbench" />
						<span className={styles.tabLabel}>Machines</span>
					</Toggle>
					<Toggle
						render={<ToggleStyles />}
						value={"repos" satisfies MeshGrouping}
						aria-label="Repos"
					>
						<Icon name="folder" />
						<span className={styles.tabLabel}>Repos</span>
					</Toggle>
				</ToggleGroup>
			</div>

			<ScrollArea className={styles.page}>
				<MeshTree projectId={projectId} grouping={grouping} />
			</ScrollArea>
		</div>
	);
};

const meshItemId = (key: string): string => `mesh-treeitem-${encodeURIComponent(key)}`;

/**
 * One tree, walked as the workspace's are: the tree holds focus, the arrow keys move the
 * selection, and selecting something in the open project shows it in the details pane.
 */
const MeshTree: FC<{ projectId: string; grouping: MeshGrouping }> = ({ projectId, grouping }) => {
	const dispatch = useAppDispatch();
	const navigate = useNavigate();
	const unfolded = useAppSelector(interfaceSlice.selectors.selectMeshUnfolded);
	const { machines, repos } = useMeshTree({ grouping, unfolded });
	const selection = useAppSelector(interfaceSlice.selectors.selectMeshSelection);
	const activeList = useActiveList();
	const ref = useRef<HTMLDivElement>(null);
	const [tooltipHandle] = useState(() => Tooltip.createHandle<FileRowTooltipPayload>());

	// Rows hold still while the pointer is over them, so a publish elsewhere can't move a target.
	const [order, setOrder] = useState<ReadonlyMap<string, number>>();
	const rows = buildMeshRows({
		machines,
		repos,
		grouping,
		unfolded,
		order,
	});
	const keys = rows.map((row) => row.key);
	const addressSpace = { items: keys, indexByKey: buildIndexByKey(keys, (key) => key) };
	const rowOf = (key: string | null): MeshRow | undefined => {
		const index = key === null ? undefined : addressSpace.indexByKey.get(key);
		return index === undefined ? undefined : rows[index];
	};
	const selectedRow = rowOf(selection);

	const openProject = (id: string, search: UrlQueryParams = {}) => {
		void navigate({ to: "/project/$id/workspace", params: { id }, search });
	};

	/**
	 * Shows what the row stands for: in the details pane for the open project, by opening any other.
	 * Opening one is left to a click or Enter, so walking the tree never leaves the project.
	 */
	const activate = (row: MeshRow, canLeave: boolean) => {
		// These show an overview instead, from the selection alone.
		if (row._tag === "Machine" || row._tag === "Repo" || row._tag === "Checkout") return;
		// A worktree's files are addressed one by one in the applied list; the first stands for them.
		const firstWorktreeFile = row._tag === "Uncommitted" ? row.worktree?.files[0] : undefined;
		const address = Match.value(row).pipe(
			Match.tags({
				Commit: ({ commit, worktree }) =>
					commitAddress({ commitId: commit.id, changeId: commit.changeId, worktree }),
				Branch: ({ branch }) => branchAddress({ branchRef: branch.ref }),
				Worktree: ({ branch }) =>
					branch === undefined ? undefined : branchAddress({ branchRef: branch.ref }),
				Uncommitted: ({ worktree }) =>
					worktree !== undefined && firstWorktreeFile !== undefined
						? fileAddress({
								parent: worktreeChangesFileParent(worktree.name),
								path: firstWorktreeFile,
							})
						: undefined,
			}),
			Match.orElse(() => undefined),
		);

		if (row.checkout.projectId === projectId) {
			if (row._tag === "Uncommitted" && row.worktree === undefined) {
				setActiveList("uncommitted");
			} else if (address !== undefined) {
				setActiveList("applied");
				setCursor("applied", address);
			}
		} else if (canLeave) {
			openProject(
				row.checkout.projectId,
				row._tag === "Uncommitted" && row.worktree === undefined
					? { active: "uncommitted" }
					: address === undefined
						? {}
						: { applied: encodeCursorParam("applied", address) ?? undefined },
			);
		}
	};

	const select = (row: MeshRow, canLeave: boolean) => {
		const overview = Match.value(row).pipe(
			Match.tags({
				Machine: ({ machine }): MeshOverview => ({ _tag: "Machine", machine: machine.name }),
				Repo: ({ repo }): MeshOverview => ({ _tag: "Repo", projectId: repo.projectId }),
				Checkout: ({ checkout }): MeshOverview => ({
					_tag: "Repo",
					projectId: checkout.projectId,
					machine: checkout.machine,
				}),
			}),
			Match.orElse(() => null),
		);
		dispatch(interfaceSlice.actions.selectMeshRow({ key: row.key, overview }));
		activate(row, canLeave);
	};

	/** A row that doesn't fold folds the one it sits in, handing it the selection. */
	const toggleFold = (row: MeshRow) => {
		if (row.folded !== undefined) {
			dispatch(interfaceSlice.actions.toggleMeshRow({ key: row.key }));
			return;
		}
		const parent = rowOf(row.parentKey);
		if (parent === undefined) return;
		select(parent, false);
		dispatch(interfaceSlice.actions.toggleMeshRow({ key: parent.key }));
	};

	useAddressSpaceHotkeys({
		projectId,
		addressSpace,
		group: "Sidebar",
		select: (key) => {
			const row = rowOf(key);
			if (row === undefined) return;
			select(row, false);
			document.getElementById(meshItemId(key))?.scrollIntoView({ block: "nearest" });
		},
		selection: selectedRow === undefined ? null : selectedRow.key,
		ref,
		getKey: (key) => key,
	});

	useHotkeys([
		{
			hotkey: sidebarHotkeys.toggleFoldBranch.hotkey,
			callback: () => {
				if (selectedRow !== undefined) toggleFold(selectedRow);
			},
			options: {
				conflictBehavior: "allow",
				enabled: selectedRow !== undefined,
				target: ref,
				meta: { group: "Sidebar", name: "Fold/unfold" },
			},
		},
		{
			hotkey: "Enter",
			callback: () => {
				if (selectedRow === undefined) return;
				if (
					selectedRow._tag === "Checkout" &&
					selectedRow.checkout.isThisMachine &&
					selectedRow.checkout.projectId !== projectId
				)
					openProject(selectedRow.checkout.projectId);
				else activate(selectedRow, true);
			},
			options: {
				conflictBehavior: "allow",
				enabled: selectedRow !== undefined,
				target: ref,
				meta: { group: "Sidebar", name: "Open" },
			},
		},
	]);

	return (
		<div
			// Taking focus while the uncommitted changes are shown would switch the details back.
			ref={useMergedRefs(ref, useAutofocusScope(activeList === "applied"))}
			tabIndex={0}
			role="tree"
			aria-label={grouping === "machines" ? "Machines" : "Repos"}
			aria-activedescendant={selectedRow === undefined ? undefined : meshItemId(selectedRow.key)}
			data-focus-scope={"sidebar" satisfies FocusScope}
			className={styles.tree}
			onPointerEnter={() => setOrder(new Map(keys.map((key, index) => [key, index])))}
			onPointerLeave={() => setOrder(undefined)}
		>
			<FileRowTooltipRoot handle={tooltipHandle} />
			<MeshWatchers projectId={projectId} grouping={grouping} unfolded={unfolded} />
			{rows.map((row) => (
				<MeshRowItem
					key={row.key}
					row={row}
					projectId={projectId}
					grouping={grouping}
					tooltipHandle={tooltipHandle}
					onSelect={() => select(row, true)}
					onOpenProject={openProject}
				/>
			))}
		</div>
	);
};

/** Drawn as the workspace's remote machines draw theirs, and read out, as the dot is only seen. */
/**
 * A machine, with a light in its corner for whether it's online; this machine has none, as being
 * here is being online. The light is read out, as it's only seen.
 */
const MachineIcon: FC<{ online?: boolean }> = ({ online }) => (
	<span className={styles.machineIcon}>
		<Icon name="workbench" size={14} />
		{online !== undefined && (
			<>
				<span aria-hidden className={classes(styles.light, online && styles.lightOn)} />
				<span className={styles.hidden}>{online ? "Online" : "Offline"}</span>
			</>
		)}
	</span>
);

const When: FC<{ at: number | null }> = ({ at }) =>
	at === null ? null : (
		<span className="text-12">
			<RelativeTime timestamp={at} compact />
		</span>
	);

const pluralize = (count: number, noun: string): string =>
	count === 1 ? `1 ${noun}` : `${count} ${noun}s`;

const copyItem = (label: string, text: string): NativeMenuItem =>
	nativeMenuItem({ label, onSelect: () => window.lite.clipboardWriteText(text) });

const MeshRowItem: FC<{
	row: MeshRow;
	projectId: string;
	grouping: MeshGrouping;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
	onOpenProject: (projectId: string) => void;
}> = ({ row, projectId, grouping, tooltipHandle, onSelect, onOpenProject }) => {
	const shared = { row, tooltipHandle, onSelect };
	const openProjectItem = (id: string) =>
		nativeMenuItem({
			label: "Open Project",
			enabled: id !== projectId,
			onSelect: () => onOpenProject(id),
		});

	switch (row._tag) {
		case "Machine":
			return (
				<MeshItem
					{...shared}
					name={row.machine.name}
					className={styles.groupItem}
					icon={<MachineIcon online={row.machine.isThisMachine ? undefined : row.machine.online} />}
					marks={<When at={row.machine.at} />}
					menuLabel="Machine menu"
					menuItems={[copyItem("Copy Machine Name", row.machine.name)]}
				/>
			);
		case "Repo":
			return (
				<MeshItem
					{...shared}
					name={row.repo.name}
					tooltip={row.repo.path ?? row.repo.name}
					className={styles.groupItem}
					marks={
						<>
							{row.repo.projectId === projectId && (
								<Badge variant="lightGray" className={styles.badge}>
									Open
								</Badge>
							)}
							<When at={row.repo.at} />
						</>
					}
					menuLabel="Repository menu"
					menuItems={[openProjectItem(row.repo.projectId)]}
				/>
			);
		case "Checkout":
			return (
				<MeshItem
					{...shared}
					name={row.name}
					tooltip={row.checkout.path ?? row.name}
					icon={
						grouping === "repos" ? (
							<MachineIcon online={row.checkout.isThisMachine ? undefined : row.checkout.online} />
						) : undefined
					}
					marks={
						<>
							{grouping === "machines" &&
								row.checkout.isThisMachine &&
								row.checkout.projectId === projectId && (
									<Badge variant="lightGray" className={styles.badge}>
										Open
									</Badge>
								)}
							<When at={row.checkout.at} />
						</>
					}
					menuLabel="Repository menu"
					menuItems={[openProjectItem(row.checkout.projectId)]}
				/>
			);
		case "Uncommitted":
			return (
				<MeshItem
					{...shared}
					name="Uncommitted changes"
					icon={<Icon name="file-diff" size={14} />}
					marks={
						<span className="text-12">
							{pluralize(row.worktree?.files.length ?? row.checkout.uncommittedFiles, "file")}
						</span>
					}
				/>
			);
		case "Worktree": {
			const { worktree, branch } = row;
			// Named as a directory, so it never reads as a branch; its branch only when it differs.
			const summary = [
				branch !== undefined && branch.name !== worktree.name ? branch.name : null,
				branch !== undefined ? pluralize(branch.commits.length, "commit") : null,
				worktree.files.length > 0 ? pluralize(worktree.files.length, "file") : null,
			].filter((part) => part !== null);
			return (
				<MeshItem
					{...shared}
					name={`${worktree.name}/`}
					icon={<Icon name="folder-tree" size={14} />}
					marks={summary.length > 0 && <span className="text-12">{summary.join(", ")}</span>}
					menuLabel="Worktree menu"
					menuItems={branch === undefined ? undefined : [copyItem("Copy Branch Name", branch.name)]}
				/>
			);
		}
		case "Branch":
			return <MeshBranchItem {...shared} checkout={row.checkout} branch={row.branch} />;
		case "Commit": {
			if (row.uncommitted) {
				return (
					<MeshItem
						{...shared}
						name="Uncommitted changes"
						icon={<Icon name="file-diff" size={14} />}
					/>
				);
			}
			const title = commitTitle(row.commit.message);
			const body = commitBody(row.commit.message);
			return (
				<MeshItem
					{...shared}
					name={title ?? "(no message)"}
					icon={<Icon name="commit" size={14} />}
					menuLabel="Commit menu"
					menuItems={[
						nativeMenuItem({
							label: "Copy",
							submenu: [
								nativeMenuItem({
									label: "Change ID",
									enabled: row.commit.changeId !== "",
									onSelect: () => window.lite.clipboardWriteText(row.commit.changeId),
								}),
								copyItem("Commit ID", row.commit.id),
								nativeMenuItem({
									label: "Commit Title",
									enabled: title !== undefined,
									onSelect: () => window.lite.clipboardWriteText(title ?? ""),
								}),
								nativeMenuItem({
									label: "Commit Body",
									enabled: body !== undefined,
									onSelect: () => window.lite.clipboardWriteText(body ?? ""),
								}),
							],
						}),
					]}
				/>
			);
		}
	}
};

/** A branch, and for one another machine sent here, pulling or dismissing it. */
const MeshBranchItem: FC<{
	row: MeshRow;
	checkout: MeshCheckout;
	branch: MeshBranch;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
}> = ({ checkout, branch, ...shared }) => {
	const { projectId, machine } = checkout;
	const { mutate: pull, isPending: isPullPending } = useHostedBranchPull(projectId);
	const { mutate: dismiss, isPending: isDismissPending } = useHostedBranchDismiss(projectId);
	const settle = useHostedSync();
	const pullInto = (intoWorkspace: boolean, overwrite = false) =>
		pull(
			{
				projectId,
				machine,
				branch: branch.name,
				intoWorkspace,
				onConflict: overwrite ? "overwrite" : null,
			},
			{
				onSuccess: (outcome) =>
					settle(outcome, {
						title: "Overwrite local work?",
						keepLabel: "Keep local",
						overwrite: () => pullInto(intoWorkspace, true),
					}),
			},
		);
	const commits = pluralize(branch.commits.length, "commit");
	const summary = branch.uncommitted !== null ? `${commits}, uncommitted` : commits;

	return (
		<MeshItem
			{...shared}
			name={branch.name}
			icon={<Icon name="branch" size={14} />}
			marks={<span className="text-12">{summary}</span>}
			menuLabel="Branch menu"
			menuItems={[
				...(branch.sent
					? [
							nativeMenuItem({
								label: "Pull into Worktree",
								enabled: !isPullPending,
								onSelect: () => pullInto(false),
							}),
							nativeMenuItem({
								label: "Pull into Workspace",
								enabled: !isPullPending,
								onSelect: () => pullInto(true),
							}),
							nativeMenuItem({
								label: "Dismiss",
								enabled: !isDismissPending,
								onSelect: () => dismiss({ projectId, machine, branch: branch.name }),
							}),
							nativeMenuSeparator,
						]
					: []),
				copyItem("Copy Branch Name", branch.name),
			]}
		/>
	);
};

/** A tree item as the files tree draws one, with the workspace rows' selection and menus. */
const MeshItem: FC<{
	row: MeshRow;
	name: string;
	/** What hovering the name shows, the name itself by default. */
	tooltip?: string;
	icon?: ReactNode;
	marks?: ReactNode;
	className?: string;
	menuLabel?: string;
	menuItems?: Array<NativeMenuItem>;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
}> = ({
	row,
	name,
	tooltip = name,
	icon,
	marks,
	className,
	menuLabel,
	menuItems,
	tooltipHandle,
	onSelect,
}) => {
	const dispatch = useAppDispatch();
	const isSelected = useAppSelector((state) =>
		interfaceSlice.selectors.selectIsMeshRowSelected(state, row.key),
	);

	return (
		// oxlint-disable-next-line jsx-a11y/interactive-supports-focus -- Focus stays on the tree, which names this row with aria-activedescendant.
		<FileListItem
			{...rowPointerProps({ onSelect })}
			id={meshItemId(row.key)}
			role="treeitem"
			aria-selected={isSelected}
			aria-expanded={row.folded === undefined ? undefined : !row.folded}
			aria-level={row.depth + 1}
			aria-posinset={row.positionInSet}
			aria-setsize={row.setSize}
			className={classes(className, row.depth === 0 && row.positionInSet > 1 && styles.groupStart)}
			depth={row.depth}
			name={name}
			icon={icon}
			marks={marks === undefined ? undefined : <span className={styles.marks}>{marks}</span>}
			selected={isSelected}
			folded={row.folded}
			onToggleFolded={() => dispatch(interfaceSlice.actions.toggleMeshRow({ key: row.key }))}
			labelRender={
				<Tooltip.Trigger handle={tooltipHandle} payload={{ content: tooltip }} render={<div />} />
			}
			actions={
				menuItems && (
					<Toolbar.Root aria-label={menuLabel}>
						<Toolbar.Button
							aria-label={menuLabel}
							onClick={(event) => void showNativeMenuFromTrigger(event.currentTarget, menuItems)}
							className={getRowButtonClassName({ iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>
				)
			}
			onContextMenu={menuItems && ((event) => void showNativeContextMenu(event, menuItems))}
		/>
	);
};
