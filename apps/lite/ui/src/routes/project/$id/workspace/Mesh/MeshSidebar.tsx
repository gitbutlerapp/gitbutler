import { commitAddress } from "#ui/addresses.ts";
import { commitBody, commitTitle } from "#ui/commit.ts";
import { encodeCursorParam, type UrlQueryParams } from "#ui/cursor-url.ts";
import { type FocusScope, useAddressSpaceHotkeys, useAutofocusScope } from "#ui/focus-scopes.ts";
import { sidebarHotkeys } from "#ui/hotkeys.ts";
import { interfaceSlice, type MeshGrouping } from "#ui/interface/state.ts";
import {
	type NativeMenuItem,
	nativeMenuItem,
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
import { useMeshTree } from "./useMeshTree.ts";
import styles from "./MeshSidebar.module.css";

/**
 * The sidebar as a mesh, like but.dev's navigator: every local project and every machine that
 * published one, grouped by machine or by repo. It only reads; the workspace sidebar commits.
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
	const { machines, repos } = useMeshTree();
	const toggled = useAppSelector(interfaceSlice.selectors.selectMeshToggled);
	const selection = useAppSelector(interfaceSlice.selectors.selectMeshSelection);
	const activeList = useActiveList();
	const ref = useRef<HTMLDivElement>(null);
	const [tooltipHandle] = useState(() => Tooltip.createHandle<FileRowTooltipPayload>());

	const rows = buildMeshRows({ machines, repos, grouping, toggled });
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
		if (row._tag !== "Checkout" && row._tag !== "Commit" && row._tag !== "Uncommitted") return;
		const address =
			row._tag === "Commit"
				? commitAddress({ commitId: row.commit.id, changeId: row.commit.changeId })
				: undefined;

		if (row.checkout.projectId === projectId) {
			if (row._tag === "Uncommitted") {
				setActiveList("uncommitted");
			} else if (address !== undefined) {
				setActiveList("applied");
				setCursor("applied", address);
			}
		} else if (canLeave) {
			openProject(
				row.checkout.projectId,
				row._tag === "Uncommitted"
					? { active: "uncommitted" }
					: address === undefined
						? {}
						: { applied: encodeCursorParam("applied", address) ?? undefined },
			);
		}
	};

	const select = (row: MeshRow, canLeave: boolean) => {
		dispatch(interfaceSlice.actions.selectMeshRow({ key: row.key }));
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
				if (selectedRow !== undefined) activate(selectedRow, true);
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
		>
			<FileRowTooltipRoot handle={tooltipHandle} />
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
const OnlineLight: FC<{ online: boolean }> = ({ online }) => (
	<>
		<span aria-hidden className={classes(styles.light, online && styles.lightOn)} />
		<span className={styles.hidden}>{online ? "Online" : "Offline"}</span>
	</>
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
					icon={<OnlineLight online={row.machine.online} />}
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
					className={styles.groupItem}
					marks={
						<>
							{row.repo.projectId === projectId && <Badge variant="lightGray">Open</Badge>}
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
					icon={grouping === "repos" ? <OnlineLight online={row.checkout.online} /> : undefined}
					marks={
						<>
							{grouping === "machines" &&
								row.checkout.isThisMachine &&
								row.checkout.projectId === projectId && <Badge variant="lightGray">Open</Badge>}
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
						<span className="text-12">{pluralize(row.checkout.uncommittedFiles, "file")}</span>
					}
				/>
			);
		case "Branch": {
			const commits = pluralize(row.branch.commits.length, "commit");
			return (
				<MeshItem
					{...shared}
					name={row.branch.name}
					icon={<Icon name="branch" size={14} />}
					marks={
						<span className="text-12">
							{row.branch.uncommitted !== null ? `${commits}, uncommitted` : commits}
						</span>
					}
					menuLabel="Branch menu"
					menuItems={[copyItem("Copy Branch Name", row.branch.name)]}
				/>
			);
		}
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

/** A tree item as the files tree draws one, with the workspace rows' selection and menus. */
const MeshItem: FC<{
	row: MeshRow;
	name: string;
	icon?: ReactNode;
	marks?: ReactNode;
	className?: string;
	menuLabel?: string;
	menuItems?: Array<NativeMenuItem>;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	onSelect: () => void;
}> = ({ row, name, icon, marks, className, menuLabel, menuItems, tooltipHandle, onSelect }) => {
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
				<Tooltip.Trigger handle={tooltipHandle} payload={{ content: name }} render={<div />} />
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
