import { queryOptions, skipToken, useQuery, useQueryClient } from "@tanstack/react-query";
import { useHotkey } from "@tanstack/react-hotkeys";
import { useSearch } from "@tanstack/react-router";
import { useRef, useState, type FC } from "react";
import type { CodeViewHandle } from "@pierre/diffs/react";
import { parsePatchFiles } from "@pierre/diffs";
import { Toolbar, Tooltip } from "@base-ui/react";
import { Button, getButtonClassName } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { EmptyState } from "@gitbutler/ui-react/EmptyState.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { FileList, FileListItem } from "@gitbutler/ui-react/FileList.tsx";
import { formatAbsoluteTime } from "@gitbutler/ui-react/time.ts";
import {
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	repositoryRootCommitQueryOptions,
} from "#ui/api/queries.ts";
import { butDevSessionQueryOptions, type ButDevSession } from "#ui/but-dev/auth.ts";
import { compareFilePaths } from "#ui/file-order.ts";
import { focusScope, getFocusedScope } from "#ui/focus-scopes.ts";
import { combineHashes, hash } from "#ui/hash.ts";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import {
	reviewedFilesQueryOptions,
	useSetFilesReviewed,
	type SetFilesReviewedInput,
} from "#ui/reviewed-files.ts";
import { defaultSettings } from "#ui/settings.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { setButDevCheckout, usePage } from "#ui/use-cursor.ts";
import { showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { ChangeStats } from "./ChangeStats.tsx";
import {
	DiffCodeView,
	DiffControls,
	DiffFileHeaderView,
	DiffPanels,
	FilesToggle,
} from "./Details.tsx";
import { itemAtViewportTop } from "./diff-view.ts";
import { VirtualFilesList } from "./FilesTree.tsx";
import { FileRowTooltipRoot, type FileRowTooltipPayload } from "./FileRowTooltip.tsx";
import { RowToolbar, SectionHeaderRow } from "./Row.tsx";
import { buildFileTreeRows, selectedFilePath } from "./file-tree.ts";
import { useFileDisplayModeMenuItems } from "./useFileDisplayModeMenuItems.ts";
import detailsStyles from "./Details.module.css";
import treeStyles from "./FilesTree.module.css";
import styles from "./ButDev.module.css";

// Only the fields this view consumes from but.dev's mesh-proto.
type Branch = { name: string; tip: string | null; commits: Array<{ id: string; title: string }> };
type Checkout = {
	id: string;
	node_id: string;
	project_id: string;
	path: string;
	head: "workspace" | "branch" | "detached" | "unborn";
	head_branch: string | null;
	head_commit: string | null;
	stacks: Array<{ branches: Array<Branch> }>;
	branch: Branch | null;
	uncommitted: Array<unknown>;
	updated_at: number;
	error: string | null;
};
type Mesh = {
	nodes: Array<{ id: string; name: string; online: boolean }>;
	projects: Array<{ id: string; name: string; root_commit: string }>;
	checkouts: Array<Checkout>;
};
type DiffFile = {
	path: string;
	status: "added" | "modified" | "deleted" | "renamed" | "untracked" | "conflicted";
	additions: number;
	deletions: number;
};
type Diff = { diff: string; truncated: boolean; source: string; files: Array<DiffFile> };
const fileStatuses = {
	added: "Addition",
	untracked: "Addition",
	modified: "Modification",
	deleted: "Deletion",
	renamed: "Rename",
	conflicted: undefined,
} as const;

const meshQueryOptions = (session: ButDevSession | null | undefined) =>
	queryOptions({
		// oxlint-disable-next-line @tanstack/query/exhaustive-deps -- Scope by account, never put credentials in query keys.
		queryKey: ["butDev", "mesh", session?.login],
		queryFn: session
			? async ({ signal }) => {
					const response = await fetch("https://but.dev/api/mesh", {
						headers: { Authorization: `Bearer ${session.token}` },
						credentials: "omit",
						redirect: "error",
						signal: AbortSignal.any([signal, AbortSignal.timeout(60_000)]),
					});
					if (!response.ok) {
						throw new Error(
							response.status === 401
								? "Reconnect to but.dev in Integrations."
								: `but.dev request failed (${response.status}).`,
						);
					}
					return response.json() as Promise<Mesh>;
				}
			: skipToken,
		staleTime: 15_000,
		refetchInterval: 30_000,
	});

const ButDevTargetRow: FC<{
	checkoutId: string;
	target: string;
	title: string;
	icon: "diff" | "branch" | "commit";
	detail?: string;
	inWorkspace?: boolean;
}> = ({ checkoutId, target, title, icon, detail, inWorkspace = false }) => {
	const selected = useSearch({
		from: "/project/$id/workspace",
		select: (search) =>
			(inWorkspace
				? search.page === undefined && search.active === "but-dev"
				: search.page === "but-dev") &&
			search.butDevCheckout === checkoutId &&
			(search.butDevTarget ?? "uncommitted") === target,
	});
	return (
		<Button
			variant={selected ? "gray" : "ghost"}
			className={styles.target}
			aria-pressed={selected}
			onClick={() => setButDevCheckout(checkoutId, target, inWorkspace)}
			title={title}
		>
			<Icon name={icon} />
			<span className={styles.path}>{title}</span>
			{detail !== undefined && <span className={styles.hint}>{detail}</span>}
		</Button>
	);
};

const ButDevBranchRows: FC<{ checkoutId: string; branch: Branch; inWorkspace?: boolean }> = ({
	checkoutId,
	branch,
	inWorkspace,
}) => (
	<div>
		<ButDevTargetRow
			checkoutId={checkoutId}
			target={`branch:${branch.name}`}
			title={branch.name}
			icon="branch"
			inWorkspace={inWorkspace}
		/>
		<div className={styles.commits}>
			{branch.commits.map((commit) => (
				<ButDevTargetRow
					key={`${branch.name}:${commit.id}`}
					checkoutId={checkoutId}
					target={commit.id}
					title={commit.title}
					icon="commit"
					detail={commit.id.slice(0, 7)}
					inWorkspace={inWorkspace}
				/>
			))}
		</div>
	</div>
);

const CheckoutRow: FC<{ checkout: Checkout; inWorkspace?: boolean }> = ({
	checkout,
	inWorkspace = false,
}) => {
	const selected = useSearch({
		from: "/project/$id/workspace",
		select: (search) =>
			(inWorkspace
				? search.page === undefined && search.active === "but-dev"
				: search.page === "but-dev") && search.butDevCheckout === checkout.id,
	});
	const [expanded, setExpanded] = useState(inWorkspace || selected);
	const branches = checkout.branch
		? [checkout.branch]
		: checkout.stacks.flatMap((stack) => stack.branches);
	return (
		<section className={styles.checkout}>
			<h4>
				<Button
					variant={selected ? "gray" : "ghost"}
					className={styles.target}
					title={checkout.path}
					aria-expanded={expanded}
					onClick={() => setExpanded(!expanded)}
				>
					<Icon name={expanded ? "chevron-down" : "chevron-right"} />
					<span className={styles.path}>{checkout.path.split(/[\\/]/).at(-1)}</span>
				</Button>
			</h4>
			<p className={classes(styles.hint, styles.path)}>
				{checkout.head === "workspace"
					? "Workspace"
					: (checkout.head_branch ??
						(checkout.head === "unborn" ? "Empty repository" : "Detached HEAD"))}
			</p>
			{expanded && (
				<>
					{(!inWorkspace || checkout.uncommitted.length > 0) && (
						<ButDevTargetRow
							checkoutId={checkout.id}
							target="uncommitted"
							title="Uncommitted changes"
							icon="diff"
							detail={String(checkout.uncommitted.length)}
							inWorkspace={inWorkspace}
						/>
					)}
					{branches.map((branch) => (
						<ButDevBranchRows
							key={branch.name}
							checkoutId={checkout.id}
							branch={branch}
							inWorkspace={inWorkspace}
						/>
					))}
					{branches.length === 0 && checkout.head_commit !== null && (
						<ButDevTargetRow
							checkoutId={checkout.id}
							target={checkout.head_commit}
							title="HEAD"
							icon="commit"
							detail={checkout.head_commit.slice(0, 7)}
							inWorkspace={inWorkspace}
						/>
					)}
				</>
			)}
		</section>
	);
};

export const ButDevElsewhere: FC<{ projectId: string }> = ({ projectId }) => {
	const page = usePage();
	const { data: session } = useQuery(butDevSessionQueryOptions);
	const enabled = !!session && page === "workspace";
	const {
		data: root,
		error: rootError,
		refetch: refetchRoot,
	} = useQuery({
		...repositoryRootCommitQueryOptions(projectId),
		enabled,
	});
	const {
		data: localCommits,
		error: localError,
		refetch: refetchLocal,
	} = useQuery({
		...headInfoQueryOptions(projectId),
		enabled,
		select: (head) =>
			new Set(
				head.stacks.flatMap((stack) =>
					stack.segments.flatMap((segment) => segment.commits.map((commit) => commit.id)),
				),
			),
	});
	const {
		data,
		error: meshError,
		refetch: refetchMesh,
	} = useQuery({
		...meshQueryOptions(session),
		enabled,
		select: (mesh) => {
			if (root == null || localCommits === undefined) return [];
			const project = mesh.projects.find((project) => project.root_commit === root);
			if (!project) return [];
			const checkouts = mesh.checkouts
				.filter((checkout) => checkout.project_id === project.id)
				.flatMap((checkout) => {
					const branches = checkout.branch
						? [checkout.branch]
						: checkout.stacks.flatMap((stack) => stack.branches);
					const remaining = branches.flatMap((branch) => {
						const tip = branch.tip ?? branch.commits[0]?.id;
						if (tip !== undefined && localCommits.has(tip)) return [];
						const commits = branch.commits.filter((commit) => !localCommits.has(commit.id));
						return commits.length > 0 || tip !== undefined ? [{ ...branch, commits }] : [];
					});
					const head =
						branches.length === 0 &&
						checkout.head_commit !== null &&
						!localCommits.has(checkout.head_commit)
							? checkout.head_commit
							: null;
					if (remaining.length === 0 && head === null && checkout.uncommitted.length === 0)
						return [];
					return [
						{ ...checkout, branch: null, stacks: [{ branches: remaining }], head_commit: head },
					];
				});
			const byNode = Object.groupBy(checkouts, (checkout) => checkout.node_id);
			return mesh.nodes.flatMap((node) => {
				const checkouts = byNode[node.id];
				return checkouts ? [{ ...node, checkouts }] : [];
			});
		},
	});
	const error = rootError ?? localError ?? meshError;
	if (!enabled || (!error && (data === undefined || data.length === 0))) return null;
	return (
		<section aria-label="Elsewhere on but.dev" className={styles.elsewhere}>
			<SectionHeaderRow label="Elsewhere" />
			<p className={styles.hint}>Reported on but.dev. Uncommitted changes may also exist here.</p>
			{error ? (
				<div role="alert">
					<p>{error.message}</p>
					<Button onClick={() => void Promise.all([refetchRoot(), refetchLocal(), refetchMesh()])}>
						Retry
					</Button>
				</div>
			) : (
				data?.map((node) => (
					<section key={node.id} className={styles.machine}>
						<h3 className="text-13 text-semibold">{node.name}</h3>
						<p className={styles.hint}>
							{node.online ? "Online" : "Offline · showing last report"}
						</p>
						{node.checkouts.map((checkout) => (
							<CheckoutRow key={checkout.id} checkout={checkout} inWorkspace />
						))}
					</section>
				))
			)}
		</section>
	);
};

export const ButDevSidebar: FC<{ className?: string }> = ({ className }) => {
	const dispatch = useAppDispatch();
	const client = useQueryClient();
	const { data: session, error: sessionError, isPending } = useQuery(butDevSessionQueryOptions);
	const { data, error, isFetching, refetch } = useQuery({
		...meshQueryOptions(session),
		select: (mesh) => {
			const byNode = Object.groupBy(mesh.checkouts, (checkout) => checkout.node_id);
			const projects = new Map(mesh.projects.map((project) => [project.id, project.name]));
			return mesh.nodes.map((node) => ({
				...node,
				projects: Object.entries(
					Object.groupBy(byNode[node.id] ?? [], (checkout) => checkout.project_id),
				).map(([id, checkouts]) => ({ id, name: projects.get(id), checkouts })),
			}));
		},
	});
	const openIntegrations = () =>
		dispatch(
			interfaceSlice.actions.openDialog({
				dialog: { _tag: "Settings", page: "global:integrations" },
			}),
		);

	return (
		<div className={classes(styles.sidebar, className)} data-focus-scope="sidebar" tabIndex={-1}>
			<div className={styles.toolbar}>
				<span className={styles.hint}>All machines and repositories</span>
				<Button variant="ghost" iconOnly aria-label="but.dev account" onClick={openIntegrations}>
					<Icon name="settings" />
				</Button>
				<Button
					variant="ghost"
					iconOnly
					aria-label="Refresh but.dev"
					disabled={!session || isFetching}
					onClick={() =>
						void client.invalidateQueries({ queryKey: meshQueryOptions(session).queryKey })
					}
				>
					<Icon name={isFetching ? "spinner" : "refresh"} />
				</Button>
			</div>
			{sessionError || error ? (
				<div className={styles.message} role="alert">
					<p>{(sessionError ?? error)?.message}</p>
					<Button
						onClick={() =>
							void (sessionError ? client.invalidateQueries(butDevSessionQueryOptions) : refetch())
						}
					>
						Retry
					</Button>
				</div>
			) : isPending ? (
				<output className={styles.message}>Loading but.dev…</output>
			) : !session ? (
				<EmptyState
					illustration="id-card"
					title="Connect to but.dev"
					description="See unfinished work across your machines"
				>
					<Button onClick={openIntegrations}>Open Integrations</Button>
				</EmptyState>
			) : !data ? (
				<output className={styles.message}>Loading machines…</output>
			) : data.length === 0 ? (
				<EmptyState
					illustration="cactus"
					title="No machines yet"
					description="Connect a machine on but.dev to see its work here"
				/>
			) : (
				data.map((node) => (
					<section key={node.id} className={styles.machine}>
						<h2 className="text-14 text-semibold">{node.name}</h2>
						<p className={styles.hint}>
							{node.online ? "Online" : "Offline · showing last report"}
						</p>
						{node.projects.length === 0 && <p className={styles.hint}>No reported checkouts</p>}
						{node.projects.map((project) => (
							<section key={project.id} className={styles.project}>
								<h3 className="text-13 text-semibold">{project.name}</h3>
								{project.checkouts?.map((checkout) => (
									<CheckoutRow key={checkout.id} checkout={checkout} />
								))}
							</section>
						))}
					</section>
				))
			)}
		</div>
	);
};

export const ButDevDetails: FC = () => {
	const { data: session, isPending: loadingSession } = useQuery(butDevSessionQueryOptions);
	const checkoutId = useSearch({
		from: "/project/$id/workspace",
		select: (search) => search.butDevCheckout,
	});
	const target = useSearch({
		from: "/project/$id/workspace",
		select: (search) => search.butDevTarget ?? "uncommitted",
	});
	const { data, error } = useQuery({
		...meshQueryOptions(session),
		select: (mesh) => {
			const checkout = mesh.checkouts.find((checkout) => checkout.id === checkoutId);
			if (!checkout) return null;
			const branches = checkout.branch
				? [checkout.branch]
				: checkout.stacks.flatMap((stack) => stack.branches);
			return {
				checkout,
				node: mesh.nodes.find((node) => node.id === checkout.node_id),
				title:
					target === "uncommitted"
						? "Uncommitted changes"
						: target.startsWith("branch:")
							? target.slice(7)
							: (branches.flatMap((branch) => branch.commits).find((commit) => commit.id === target)
									?.title ?? target.slice(0, 7)),
			};
		},
	});
	if (error) {
		return (
			<p className={styles.message} role="alert">
				{error.message}
			</p>
		);
	}
	if (loadingSession || (session && checkoutId !== undefined && data === undefined))
		return <output className={styles.message}>Loading checkout…</output>;
	if (!session || !data) {
		return (
			<div className={styles.empty}>
				<EmptyState
					title="Work across your machines"
					description={
						!session
							? "Connect to but.dev in Integrations"
							: checkoutId === undefined
								? "Choose a checkout to see its changes"
								: "This checkout is no longer available"
					}
				/>
			</div>
		);
	}
	return (
		<CheckoutDetails
			key={`${session.login}:${data.checkout.id}:${target}`}
			session={session}
			target={target}
			{...data}
		/>
	);
};

const CheckoutDetails: FC<{
	session: ButDevSession;
	checkout: Checkout;
	node: Mesh["nodes"][number] | undefined;
	target: string;
	title: string;
}> = ({ session, checkout, node, target, title }) => {
	const [selection, setSelection] = useState<string | null>(null);
	const [collapsedDirectories, setCollapsedDirectories] = useState<Record<string, true>>({});
	const [collapsedFiles, setCollapsedFiles] = useState<Record<string, boolean>>({});
	const [filesVisible, setFilesVisible] = useState(true);
	const [tooltipHandle] = useState(() => Tooltip.createHandle<FileRowTooltipPayload>());
	const viewerRef = useRef<CodeViewHandle<undefined>>(null);
	const didScrollToFileRef = useRef(false);
	const treeRef = useRef<HTMLDivElement>(null);
	const reviewScope = { login: session.login, checkoutId: checkout.id, target };
	const { data: reviewedFiles } = useQuery({
		...reviewedFilesQueryOptions(reviewScope),
		throwOnError: true,
	});
	const { mutate: setFilesReviewed } = useSetFilesReviewed();
	const noDialog = useAppSelector(
		(state) => interfaceSlice.selectors.selectDialogState(state)._tag === "None",
	);
	const toggleFiles = () => {
		if (filesVisible && getFocusedScope(document.activeElement) === "files") focusScope("diff");
		setFilesVisible(!filesVisible);
	};
	useHotkey(workspaceHotkeys.toggleFiles.hotkey, toggleFiles, {
		enabled: noDialog,
		conflictBehavior: "allow",
		meta: workspaceHotkeys.toggleFiles.meta,
	});
	const fileDisplayModeMenuItems = useFileDisplayModeMenuItems();
	const { data: settings } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => ({
			filesOnRight: cfg.filesPanelRight ?? defaultSettings.filesPanelRight,
			mode: cfg.fileDisplayMode ?? defaultSettings.fileDisplayMode,
			pathFirst: cfg.pathFirst ?? defaultSettings.pathFirst,
			allFiles: cfg.unidiff ?? defaultSettings.unidiff,
		}),
	});
	const mode = settings?.mode ?? defaultSettings.fileDisplayMode;
	const allFiles = settings?.allFiles ?? defaultSettings.unidiff;
	const { data, error, refetch } = useQuery({
		// oxlint-disable-next-line @tanstack/query/exhaustive-deps -- Scope by account, never put credentials in query keys.
		queryKey: ["butDev", "mesh", session.login, "diff", checkout.id, target],
		queryFn: async ({ signal }) => {
			const response = await fetch(
				`https://but.dev/api/checkouts/${encodeURIComponent(checkout.id)}/diff?target=${encodeURIComponent(target)}`,
				{
					headers: { Authorization: `Bearer ${session.token}` },
					credentials: "omit",
					redirect: "error",
					signal: AbortSignal.any([signal, AbortSignal.timeout(60_000)]),
				},
			);
			if (!response.ok) {
				throw new Error(
					response.status === 401
						? "Reconnect to but.dev in Integrations."
						: `Could not read this diff (${response.status}). The machine may be unavailable.`,
				);
			}
			const diff = (await response.json()) as Diff;
			const version = hash(diff.diff);
			const items = parsePatchFiles(diff.diff, `${checkout.id}:${target}:${version}`, true)
				.flatMap((patch) => patch.files)
				.sort((a, b) => compareFilePaths(a.name, b.name))
				.map((fileDiff) => ({
					type: "diff" as const,
					id: fileDiff.name,
					fileDiff,
					// The parser's cache key covers the whole patch; reviews cover one file.
					version: hash(JSON.stringify({ ...fileDiff, cacheKey: undefined })),
				}));
			return {
				...diff,
				filesByPath: Object.fromEntries(diff.files.map((file) => [file.path, file])),
				items,
				reviewFiles: items.map(({ id, version }) => ({ path: id, version })),
				lineStats: diff.files.reduce(
					(stats, file) => ({
						linesAdded: stats.linesAdded + file.additions,
						linesRemoved: stats.linesRemoved + file.deletions,
					}),
					{ linesAdded: 0, linesRemoved: 0 },
				),
			};
		},
		staleTime: 15_000,
		refetchInterval: 30_000,
		select: (diff) => {
			const rows = buildFileTreeRows({ items: diff.files, mode, collapsedDirectories });
			const preferred = selection ?? diff.items[0]?.fileDiff.name ?? diff.files[0]?.path;
			const exactIndex = rows.findIndex((row) => row.path === preferred);
			const selectedIndex = Math.max(
				0,
				exactIndex >= 0
					? exactIndex
					: rows.findLastIndex(
							(row) => row._tag === "Directory" && preferred?.startsWith(`${row.path}/`),
						),
			);
			const selectedRow = rows[selectedIndex]?.path ?? null;
			const selectedFile =
				preferred !== undefined && diff.filesByPath[preferred] !== undefined
					? preferred
					: selectedFilePath(rows, selectedRow);
			const reviewStates = Object.fromEntries(
				diff.items.map(({ id, version }) => [
					id,
					reviewedFiles?.get(id)?.has(version)
						? ("reviewed" as const)
						: reviewedFiles?.has(id)
							? ("changed" as const)
							: null,
				]),
			);
			return {
				...diff,
				rows,
				selectedIndex,
				selectedRow,
				selectedFile,
				reviewStates,
				allFilesReviewed:
					diff.items.length > 0 && diff.items.every((item) => reviewStates[item.id] === "reviewed"),
				visibleItems: (allFiles
					? diff.items
					: diff.items.filter((item) => item.fileDiff.name === selectedFile)
				).map((item) =>
					(collapsedFiles[item.id] ?? reviewStates[item.id] === "reviewed")
						? {
								...item,
								collapsed: true,
								version: combineHashes(item.version, item.id === selectedFile ? 2 : 1),
							}
						: item,
				),
			};
		},
	});

	const activateFile = (path: string) => {
		setSelection(path);
		if (data && allFiles) {
			const file = selectedFilePath(data.rows, path);
			const item = data.items.find((item) => item.fileDiff.name === file);
			if (item && viewerRef.current) {
				const viewer = viewerRef.current.getInstance();
				const before = viewer?.getScrollTop();
				didScrollToFileRef.current = true;
				viewerRef.current.scrollTo({ type: "item", id: item.id });
				// A no-op scroll emits no event; don't suppress the next manual scroll.
				requestAnimationFrame(() => {
					if (viewer?.getScrollTop() === before) didScrollToFileRef.current = false;
				});
			}
		}
	};
	const toggleDirectory = (path: string) => {
		setSelection(path);
		setCollapsedDirectories((previous) => {
			const next = { ...previous };
			if (next[path]) delete next[path];
			else next[path] = true;
			return next;
		});
	};
	const markReviewed = (files: SetFilesReviewedInput["files"], reviewed: boolean) => {
		setFilesReviewed({ ...reviewScope, files, reviewed });
		setCollapsedFiles((previous) => {
			const next = { ...previous };
			for (const file of files) delete next[file.path];
			return next;
		});
		// Keep the header in view after reviewing folds the file.
		const path = files.length === 1 ? files[0]?.path : data?.selectedFile;
		if (path != null) requestAnimationFrame(() => activateFile(path));
	};
	const filesPanel =
		filesVisible && reviewedFiles !== undefined && data && data.files.length > 0 ? (
			<div className={detailsStyles.filesPanelContent}>
				<FileList
					className={detailsStyles.diffFiles}
					title="Changes"
					count={data.files.length}
					added={data.lineStats.linesAdded}
					removed={data.lineStats.linesRemoved}
					actions={
						<RowToolbar forceVisible>
							<Button
								variant="ghost"
								iconOnly
								aria-label="File display"
								onClick={(event) =>
									void showNativeMenuFromTrigger(event.currentTarget, fileDisplayModeMenuItems)
								}
							>
								<Icon name="kebab" />
							</Button>
						</RowToolbar>
					}
				>
					<FileRowTooltipRoot handle={tooltipHandle} />
					<div
						ref={treeRef}
						className={treeStyles.tree}
						role="tree"
						aria-label="Changed files"
						tabIndex={0}
						data-focus-scope="files"
						aria-activedescendant={
							data.selectedRow === null
								? undefined
								: `but-dev-file-${encodeURIComponent(data.selectedRow)}`
						}
						onKeyDown={(event) => {
							if (event.altKey || event.ctrlKey || event.metaKey) return;
							const index =
								event.key === "ArrowDown"
									? data.selectedIndex + 1
									: event.key === "ArrowUp"
										? data.selectedIndex - 1
										: event.key === "Home"
											? 0
											: event.key === "End"
												? data.rows.length - 1
												: null;
							if (index === null) return;
							event.preventDefault();
							const row = data.rows[Math.max(0, Math.min(index, data.rows.length - 1))];
							if (row) activateFile(row.path);
						}}
					>
						<VirtualFilesList rows={data.rows} selectedRowIndex={data.selectedIndex}>
							{({ row, index, height, measureElement }) => (
								<div
									key={row.path}
									id={`but-dev-file-${encodeURIComponent(row.path)}`}
									role="treeitem"
									aria-selected={row.path === data.selectedRow}
									aria-level={row.depth + 1}
									aria-posinset={row.positionInSet}
									aria-setsize={row.setSize}
									aria-expanded={
										row._tag === "Directory" ? !collapsedDirectories[row.path] : undefined
									}
									data-index={index}
									ref={measureElement}
									style={{ position: "absolute", top: 0, left: 0, width: "100%", height }}
								>
									<FileListItem
										name={
											row._tag === "Directory"
												? row.name
												: row.path.slice(row.path.lastIndexOf("/") + 1)
										}
										directory={
											mode === "list" && row.path.includes("/")
												? row.path.slice(0, row.path.lastIndexOf("/"))
												: undefined
										}
										directoryPosition={settings?.pathFirst ? "lead" : "trail"}
										depth={row.depth}
										folded={
											row._tag === "Directory" ? collapsedDirectories[row.path] === true : undefined
										}
										onToggleFolded={
											row._tag === "Directory" ? () => toggleDirectory(row.path) : undefined
										}
										selected={row.path === data.selectedRow}
										status={row._tag === "File" ? fileStatuses[row.item.status] : undefined}
										conflicted={row._tag === "File" && row.item.status === "conflicted"}
										reviewed={data.reviewStates[row.path] === "reviewed"}
										labelRender={
											<Tooltip.Trigger
												handle={tooltipHandle}
												payload={{ content: row.path }}
												render={<div />}
											/>
										}
										onClick={() => {
											activateFile(row.path);
											treeRef.current?.focus();
										}}
									/>
								</div>
							)}
						</VirtualFilesList>
					</div>
				</FileList>
			</div>
		) : null;

	return (
		<div className={styles.details}>
			<header className={styles.header}>
				<h1 className="text-15 text-semibold">{title}</h1>
				<p className={styles.hint}>
					{node?.name} · {checkout.path} · {node?.online ? "Online" : "Offline"} · Reported{" "}
					{formatAbsoluteTime(checkout.updated_at)}
				</p>
				{checkout.error !== null && <p role="alert">{checkout.error}</p>}
				{data && (
					<p className={styles.hint}>
						Read-only · Source: {data.source} · {data.files.length} changed files
					</p>
				)}
				{data?.truncated && <p>This diff is truncated; some changes are not shown.</p>}
			</header>
			<DiffPanels
				layoutId="but.dev:details"
				files={filesPanel}
				filesOnRight={settings?.filesOnRight ?? defaultSettings.filesPanelRight}
			>
				<div className={detailsStyles.actions}>
					<FilesToggle visible={filesVisible} onToggle={toggleFiles} />
					{!filesVisible && data && (
						<ChangeStats fileCount={data.files.length} lineStats={data.lineStats} />
					)}
					<DiffControls>
						<Toolbar.Button
							className={getButtonClassName({ variant: "outline" })}
							disabled={
								!data ||
								reviewedFiles === undefined ||
								data.truncated ||
								data.items.length === 0 ||
								data.items.length !== data.files.length
							}
							onClick={() => data && markReviewed(data.reviewFiles, !data.allFilesReviewed)}
						>
							{data?.allFilesReviewed ? "Mark all unreviewed" : "Mark all reviewed"}
						</Toolbar.Button>
					</DiffControls>
				</div>
				<div className={detailsStyles.diffContentsContainer} data-focus-scope="diff" tabIndex={-1}>
					{error ? (
						<div className={styles.message} role="alert">
							<p>{error.message}</p>
							<Button onClick={() => void refetch()}>Retry</Button>
						</div>
					) : !data || reviewedFiles === undefined ? (
						<output className={styles.message}>Loading diff…</output>
					) : data.visibleItems.length === 0 ? (
						<div className={styles.empty}>
							<EmptyState
								title={
									data.files.length === 0 && !data.truncated
										? "No changes"
										: "No text diff available"
								}
							/>
						</div>
					) : (
						<DiffCodeView
							key={allFiles ? "all" : data.selectedFile}
							ref={viewerRef}
							items={data.visibleItems}
							onScroll={(scrollTop, viewer) => {
								if (didScrollToFileRef.current) {
									didScrollToFileRef.current = false;
									return;
								}
								if (!allFiles) return;
								const item = itemAtViewportTop(scrollTop, viewer, data.visibleItems);
								if (item) setSelection(item.id);
							}}
							renderCustomHeader={(item) => {
								const file = data.filesByPath[item.id];
								return (
									<DiffFileHeaderView
										path={item.id}
										hasDiff={item.type === "diff" && item.fileDiff.hunks.length > 0}
										collapsed={item.collapsed ?? false}
										selected={item.id === data.selectedFile}
										reviewState={data.reviewStates[item.id] ?? null}
										setReviewed={(reviewed) =>
											markReviewed(
												data.reviewFiles.filter((file) => file.path === item.id),
												reviewed,
											)
										}
										lineStats={
											file ? { linesAdded: file.additions, linesRemoved: file.deletions } : null
										}
										setCollapsed={(collapsed) => {
											setCollapsedFiles((previous) => ({ ...previous, [item.id]: collapsed }));
											// Reveal the header after the new folded height reaches CodeView.
											requestAnimationFrame(() => activateFile(item.id));
										}}
									/>
								);
							}}
						/>
					)}
				</div>
			</DiffPanels>
		</div>
	);
};
