import { signOut } from "#ui/hosted-session.ts";
import {
	branchAddress,
	commitAddress,
	fileAddress,
	worktreeChangesFileParent,
} from "#ui/addresses.ts";
import { Match } from "effect";
import { commitBody, commitForgeUrl, commitTitle } from "#ui/commit.ts";
import { encodeCursorParam, type UrlQueryParams } from "#ui/cursor-url.ts";
import { type FocusScope, useAddressSpaceHotkeys, useAutofocusScope } from "#ui/focus-scopes.ts";
import { sidebarHotkeys } from "#ui/hotkeys.ts";
import { interfaceSlice, type MeshGrouping, type MeshOverview } from "#ui/interface/state.ts";
import {
	useBranchCreate,
	useCommitDiscard,
	useCommitInsertBlank,
	useCommitUncommit,
	useEnterEditMode,
	useHostedBranchDismiss,
	useHostedBranchPull,
} from "#ui/api/mutations.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { useHostedSync } from "#ui/HostedSync.tsx";
import {
	type NativeMenuContext,
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
import type { Commit, PublishState } from "@gitbutler/but-sdk";
import { Badge } from "@gitbutler/ui-react/Badge.tsx";
import { Tooltip as HintTooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { FileListItem } from "@gitbutler/ui-react/FileList.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { RelativeTime } from "@gitbutler/ui-react/RelativeTime.tsx";
import { ScrollArea } from "@gitbutler/ui-react/ScrollArea.tsx";
import { ToggleGroupStyles, ToggleStyles } from "@gitbutler/ui-react/ToggleGroup.tsx";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { type FC, type ReactNode, useRef, useState } from "react";
import { FileRowTooltipRoot, type FileRowTooltipPayload } from "../FileRowTooltip.tsx";
import { getRowButtonClassName, rowPointerProps } from "../Row-utils.ts";
import { SidebarHeader } from "../SidebarHeader.tsx";
import { useFetchFromRemotes } from "../useFetchFromRemotes.ts";
import { useAddLocalRepository } from "#ui/components/useAddLocalRepository.ts";
import { buildMeshRows, type MeshRow } from "./mesh-rows.ts";
import {
	isRemoteOnlyId,
	type MeshBranch,
	type MeshCheckout,
	type MeshWorktree,
	useMeshTree,
} from "./useMeshTree.ts";
import { useHubSendMenu } from "./useHubSendMenu.ts";
import { usePublishMenu } from "./usePublishMenu.ts";
import { MeshWatchers } from "./MeshWatchers.tsx";
import { insertBlankCommitMenuItem } from "../WorkspaceLists/insertBlankCommitMenuItem.ts";
import styles from "./MeshSidebar.module.css";

/**
 * The sidebar as a mesh, like but.dev's navigator: every local project and every machine that
 * published one, grouped by machine or by repo.
 */
export const MeshSidebar: FC<{ projectId: string }> = ({ projectId }) => {
	const dispatch = useAppDispatch();
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const fetchFromRemotes = useFetchFromRemotes(projectId);
	const grouping = useAppSelector(interfaceSlice.selectors.selectMeshGrouping);
	const { addLocalRepository, isPending: isAddingRepository } = useAddLocalRepository();
	const hosted = window.lite.hosted === true;

	return (
		<div className={styles.container}>
			<div className={styles.top}>
				{/* A hub has no local workspace: nothing to add, undo or set, only an account. */}
				{hosted ? (
					<SidebarHeader
						isFetchPending={false}
						canOpenOperationsLog={false}
						canOpenSettings={false}
						onSignOut={signOut}
					/>
				) : (
					<SidebarHeader
						bell={<NotificationBell projectId={projectId} />}
						isFetchPending={fetchFromRemotes.isPending}
						onAddRepository={() => void addLocalRepository()}
						isAddingRepository={isAddingRepository}
						canOpenOperationsLog={noOperationPending}
						onOpenOperationsLog={() =>
							dispatch(
								interfaceSlice.actions.openDialog({ dialog: { _tag: "OperationsLogPicker" } }),
							)
						}
						canOpenSettings={noOperationPending}
						onOpenSettings={() =>
							dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "Settings" } }))
						}
					/>
				)}
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
				Repo: ({ repo }): MeshOverview | null =>
					isRemoteOnlyId(repo.projectId) ? null : { _tag: "Repo", projectId: repo.projectId },
				Checkout: ({ checkout }): MeshOverview | null =>
					checkout.remoteOnly
						? null
						: {
								_tag: "Repo",
								projectId: checkout.projectId,
								machine: checkout.machine,
							},
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
const MachineIcon: FC<{ online?: boolean }> = ({ online }) => {
	const icon = (
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
	return online === undefined ? (
		icon
	) : (
		<HintTooltip content={online ? "Online: Lite is running there" : "Offline"}>{icon}</HintTooltip>
	);
};

/**
 * How a branch of this machine's compares with what it last published, as a dim globe: alone when
 * the same, with the count it's ahead or behind, tinted when the two have diverged.
 */
const PublishMark: FC<{ state: PublishState | null }> = ({ state }) => {
	if (state === null) return null;
	const [count, label] = Match.value(state).pipe(
		Match.when({ type: "published" }, () => [null, "Published"] as const),
		Match.when(
			{ type: "ahead" },
			({ subject }) =>
				[`↑${subject}`, `${pluralize(subject, "commit")} not published yet`] as const,
		),
		Match.when(
			{ type: "behind" },
			({ subject }) =>
				[`↓${subject}`, `Published with ${pluralize(subject, "commit")} not here`] as const,
		),
		Match.when({ type: "diverged" }, () => [null, "Differs from what was published"] as const),
		Match.exhaustive,
	);
	return (
		<HintTooltip content={label}>
			<span className={classes(styles.publish, state.type === "diverged" && styles.publishWarn)}>
				<Icon name="globe" size={12} />
				{count !== null && <span className="text-12">{count}</span>}
				<span className={styles.hidden}>{label}</span>
			</span>
		</HintTooltip>
	);
};

const When: FC<{ at: number | null }> = ({ at }) =>
	at === null ? null : (
		<span className="text-12">
			<RelativeTime timestamp={at} compact />
		</span>
	);

const pluralize = (count: number, noun: string, nouns = `${noun}s`): string =>
	count === 1 ? `1 ${noun}` : `${count} ${nouns}`;

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
							{row.checkout.remoteOnly ? (
								<span className="text-12">Not on this machine</span>
							) : (
								row.checkout.branches.length === 0 &&
								row.checkout.branchCount !== null && (
									<span className="text-12">
										{pluralize(row.checkout.branchCount, "branch", "branches")}
									</span>
								)
							)}
							<When at={row.checkout.at} />
						</>
					}
					menuLabel="Repository menu"
					menuItems={
						row.checkout.remoteOnly ? undefined : [openProjectItem(row.checkout.projectId)]
					}
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
		case "Worktree":
			return (
				<MeshWorktreeItem
					{...shared}
					checkout={row.checkout}
					worktree={row.worktree}
					branch={row.branch}
				/>
			);
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
			return <MeshCommitItem {...shared} checkout={row.checkout} commit={row.commit} />;
		}
	}
};

const MeshCommitItem: FC<{
	row: MeshRow;
	checkout: MeshCheckout;
	commit: Commit;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
}> = ({ checkout, commit, ...shared }) => {
	const { data: forgeInfo } = useQuery({
		...forgeInfoOptions(checkout.projectId),
		enabled: !checkout.remoteOnly,
	});
	const forgeUrl = forgeInfo && commitForgeUrl(commit, forgeInfo);
	const title = commitTitle(commit.message);
	const body = commitBody(commit.message);
	const projectId = checkout.projectId;
	// This machine's branches outside a linked worktree are the workspace's.
	const inWorkspace =
		checkout.isThisMachine && shared.row._tag === "Commit" && shared.row.worktree === undefined;
	const { data: stackId } = useQuery({
		...headInfoQueryOptions(projectId),
		enabled: inWorkspace,
		select: (headInfo) =>
			headInfo.stacks.find((stack) =>
				stack.segments.some((segment) => segment.commits.some(({ id }) => id === commit.id)),
			)?.id ?? null,
	});
	const { mutate: commitInsertBlank } = useCommitInsertBlank();
	const { isPending: isDiscardPending, mutate: commitDiscard } = useCommitDiscard();
	const { isPending: isUncommitPending, mutate: commitUncommit } = useCommitUncommit();
	const { mutate: enterEditMode } = useEnterEditMode(projectId);
	const { mutate: branchCreate } = useBranchCreate();
	const relativeTo = { type: "commit", subject: commit.id } as const;
	const createBranch = (side: "above" | "below") =>
		branchCreate({
			projectId,
			newRef: null,
			placement: { type: "dependent", subject: { relativeTo, side } },
		});
	const workspaceItems: Array<NativeMenuItem> = inWorkspace
		? [
				nativeMenuItem({
					label: "Edit Commit",
					enabled: stackId != null,
					onSelect: () => {
						if (stackId != null) enterEditMode({ projectId, commitId: commit.id, stackId });
					},
				}),
				insertBlankCommitMenuItem(
					(side) => commitInsertBlank({ projectId, relativeTo, side, dryRun: false }),
					"above",
				),
				nativeMenuSeparator,
				nativeMenuItem({
					label: "Create Branch",
					submenu: [
						nativeMenuItem({ label: "Above", onSelect: () => createBranch("above") }),
						nativeMenuItem({ label: "Below", onSelect: () => createBranch("below") }),
					],
				}),
				nativeMenuSeparator,
				nativeMenuItem({
					label: "Delete Commit",
					enabled: !isDiscardPending,
					onSelect: () =>
						commitDiscard({ projectId, subjectCommitIds: [commit.id], dryRun: false }),
				}),
				nativeMenuItem({
					label: "Uncommit",
					enabled: !isUncommitPending,
					onSelect: () =>
						commitUncommit({
							projectId,
							assignTo: null,
							subjectCommitIds: [commit.id],
							dryRun: false,
						}),
				}),
			]
		: [];
	return (
		<MeshItem
			{...shared}
			name={title ?? "(no message)"}
			icon={<Icon name="commit" size={14} />}
			menuLabel="Commit menu"
			menuContext={
				checkout.isThisMachine ? { changeId: commit.changeId, commitId: commit.id } : undefined
			}
			menuItems={[
				nativeMenuItem({
					label: "Copy",
					submenu: [
						nativeMenuItem({
							label: "Change ID",
							enabled: commit.changeId !== "",
							onSelect: () => window.lite.clipboardWriteText(commit.changeId),
						}),
						copyItem("Commit ID", commit.id),
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
				nativeMenuItem({
					label: forgeUrl?.freshness === "stale" ? "Open In Browser (stale)" : "Open In Browser",
					enabled: forgeUrl != null,
					onSelect: () => {
						if (forgeUrl) void window.lite.openInWebBrowser(forgeUrl.url);
					},
				}),
				...(workspaceItems.length > 0 ? [nativeMenuSeparator, ...workspaceItems] : []),
			]}
		/>
	);
};

/** A linked worktree, named as a directory so it never reads as a branch. */
const MeshWorktreeItem: FC<{
	row: MeshRow;
	checkout: MeshCheckout;
	worktree: MeshWorktree;
	/** Its only branch, shown on this row. */
	branch?: MeshBranch;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
}> = ({ checkout, worktree, branch, ...shared }) => {
	const publishItems = usePublishMenu({
		projectId: checkout.projectId,
		branch: branch?.name ?? "",
		local: checkout.isThisMachine && branch !== undefined,
		inWorktree: true,
		state: branch?.publishState ?? null,
	});
	// Its branch only when it differs from the directory's name.
	const summary = [
		branch !== undefined && branch.name !== worktree.name ? branch.name : null,
		branch !== undefined ? pluralize(branch.commits.length, "commit") : null,
		worktree.files.length > 0 ? pluralize(worktree.files.length, "file") : null,
	].filter((part) => part !== null);
	return (
		<MeshItem
			{...shared}
			name={`${worktree.name}/`}
			icon={
				<HintTooltip content="Linked worktree">
					<span className={styles.machineIcon}>
						<Icon name="folder-tree" size={14} />
					</span>
				</HintTooltip>
			}
			marks={
				<>
					{summary.length > 0 && <span className="text-12">{summary.join(", ")}</span>}
					<PublishMark state={branch?.publishState ?? null} />
				</>
			}
			menuLabel="Worktree menu"
			menuItems={
				branch === undefined
					? undefined
					: [...publishItems, copyItem("Copy Branch Name", branch.name)]
			}
		/>
	);
};

/** A branch: this machine's to publish or send, another machine's to pull. */
const MeshBranchItem: FC<{
	row: MeshRow;
	checkout: MeshCheckout;
	branch: MeshBranch;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
}> = (props) =>
	// Apart, so a row only holds the queries and mutations its own menu uses.
	props.checkout.isThisMachine ? <LocalBranchItem {...props} /> : <RemoteBranchItem {...props} />;

const branchSummary = (branch: MeshBranch) => {
	const commits = pluralize(branch.commits.length, "commit");
	return branch.uncommitted !== null ? `${commits}, uncommitted` : commits;
};

const LocalBranchItem: FC<{
	row: MeshRow;
	checkout: MeshCheckout;
	branch: MeshBranch;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
}> = ({ checkout, branch, ...shared }) => {
	const publishItems = usePublishMenu({
		projectId: checkout.projectId,
		branch: branch.name,
		local: true,
		inWorktree: branch.worktree !== undefined,
		state: branch.publishState,
	});
	return (
		<MeshItem
			{...shared}
			name={branch.name}
			icon={<Icon name="branch" size={14} />}
			marks={
				<>
					<span className="text-12">{branchSummary(branch)}</span>
					<PublishMark state={branch.publishState} />
				</>
			}
			menuLabel="Branch menu"
			menuItems={[...publishItems, copyItem("Copy Branch Name", branch.name)]}
			menuContext={{ branchRef: decodeBytes(branch.ref) }}
		/>
	);
};

/** Another machine's branch, to pull here, or, if it was sent here, to dismiss. */
const RemoteBranchItem: FC<{
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
	// The hosted page only reads; pulling is for machines.
	const canPull = window.lite.hosted !== true && !checkout.remoteOnly;
	const sendItems = useHubSendMenu({ projectId, from: machine, branch: branch.name });

	return (
		<MeshItem
			{...shared}
			name={branch.name}
			icon={<Icon name="branch" size={14} />}
			marks={<span className="text-12">{branchSummary(branch)}</span>}
			menuLabel="Branch menu"
			menuItems={[
				...sendItems,
				...(canPull
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
							...(branch.sent
								? [
										nativeMenuItem({
											label: "Dismiss",
											enabled: !isDismissPending,
											onSelect: () => dismiss({ projectId, machine, branch: branch.name }),
										}),
									]
								: []),
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
	/** What the menu is for, which a browser's menu offers to open in the app. */
	menuContext?: NativeMenuContext;
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
	menuContext,
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
							onClick={(event) =>
								void showNativeMenuFromTrigger(event.currentTarget, menuItems, menuContext)
							}
							className={getRowButtonClassName({ iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>
				)
			}
			onContextMenu={
				menuItems && ((event) => void showNativeContextMenu(event, menuItems, menuContext))
			}
		/>
	);
};
