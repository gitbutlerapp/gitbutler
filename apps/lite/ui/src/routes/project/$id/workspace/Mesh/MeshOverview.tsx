import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import type { FC, ReactNode } from "react";
import { useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import {
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	hostedMachinesQueryOptions,
	hostedPresenceQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { interfaceSlice, type MeshOverview as Overview } from "#ui/interface/state.ts";
import { defaultSettings } from "#ui/settings.ts";
import { useAppSelector } from "#ui/store.ts";
import type { BottomUpdate } from "@gitbutler/but-sdk";
import { Badge } from "@gitbutler/ui-react/Badge.tsx";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { RelativeTime } from "@gitbutler/ui-react/RelativeTime.tsx";
import { ScrollArea } from "@gitbutler/ui-react/ScrollArea.tsx";
import { useMeshTree } from "./useMeshTree.ts";
import styles from "./MeshOverview.module.css";

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** What the details pane shows for a mesh row that has no diff: a machine, or a repo. */
export const MeshOverview: FC<{ overview: Overview; projectId: string }> = ({
	overview,
	projectId,
}) => (
	<ScrollArea className={styles.host}>
		<div className={styles.page}>
			{overview._tag === "Machine" ? (
				<MachineOverview machine={overview.machine} projectId={projectId} />
			) : (
				<RepoOverview
					repoProjectId={overview.projectId}
					machine={overview.machine}
					projectId={projectId}
				/>
			)}
		</div>
	</ScrollArea>
);

const Section: FC<{ heading: string; children: ReactNode }> = ({ heading, children }) => (
	<section className={styles.section}>
		<h3 className={classes("text-12", "text-semibold", styles.heading)}>{heading}</h3>
		{children}
	</section>
);

const Item: FC<{ title: ReactNode; detail?: ReactNode; action?: ReactNode }> = ({
	title,
	detail,
	action,
}) => (
	<div className={styles.item}>
		<div className={styles.itemText}>
			<span className={classes("text-13", styles.itemTitle)}>{title}</span>
			{detail !== undefined && <span className={classes("text-12", styles.muted)}>{detail}</span>}
		</div>
		{action}
	</div>
);

const useOpenProject = () => {
	const navigate = useNavigate();
	return (id: string) => void navigate({ to: "/project/$id/workspace", params: { id } });
};

/** A machine: whether it's online, when it last published, and what it has of each repo. */
const MachineOverview: FC<{ machine: string; projectId: string }> = ({ machine, projectId }) => {
	const grouping = useAppSelector(interfaceSlice.selectors.selectMeshGrouping);
	const unfolded = useAppSelector(interfaceSlice.selectors.selectMeshUnfolded);
	const { machines } = useMeshTree({ grouping, unfolded });
	const openProject = useOpenProject();
	const found = machines.find((candidate) => candidate.name === machine);
	if (found === undefined)
		return <h2 className={classes("text-15", "text-semibold")}>{machine}</h2>;

	return (
		<>
			<header className={styles.header}>
				<h2 className={classes("text-15", "text-semibold")}>{found.name}</h2>
				<span className={classes("text-12", styles.muted)}>
					{found.isThisMachine ? (
						count(found.checkouts.length, "repository", "repositories")
					) : (
						<>
							{found.online ? "Online" : "Offline"}
							{found.at !== null && (
								<>
									{" · last published "}
									<RelativeTime timestamp={found.at} />
								</>
							)}
						</>
					)}
				</span>
			</header>
			<Section heading={found.isThisMachine ? "Repositories" : "Published"}>
				{found.checkouts.map((checkout) => {
					const branches =
						checkout.branches.length + checkout.worktrees.length || (checkout.branchCount ?? 0);
					return (
						<Item
							key={checkout.projectId}
							title={
								<>
									{checkout.repo}
									{checkout.projectId === projectId && <Badge variant="lightGray">Open</Badge>}
								</>
							}
							detail={
								found.isThisMachine
									? checkout.path
									: branches > 0
										? count(branches, "branch", "branches")
										: undefined
							}
							action={
								found.isThisMachine &&
								checkout.projectId !== projectId && (
									<Button size="small" onClick={() => openProject(checkout.projectId)}>
										Open
									</Button>
								)
							}
						/>
					);
				})}
			</Section>
		</>
	);
};

/**
 * A repo: where its base stands, which machines have it, and its worktrees here. With `machine`,
 * that machine comes first.
 */
const RepoOverview: FC<{ repoProjectId: string; machine?: string; projectId: string }> = ({
	repoProjectId,
	machine,
	projectId,
}) => {
	const local = window.lite.hosted !== true;
	const { data: project } = useQuery({
		...listProjectsQueryOptions,
		select: (projects) => projects.find((candidate) => candidate.id === repoProjectId),
	});
	const { data: hostedEnabled = false } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) =>
			window.lite.hosted === true || (cfg.hostedBranches ?? defaultSettings.hostedBranches),
	});
	const { data: published = [] } = useQuery({
		...hostedMachinesQueryOptions(repoProjectId),
		enabled: hostedEnabled,
	});
	const { data: online = [] } = useQuery(hostedPresenceQueryOptions(repoProjectId));
	const { data: headInfo } = useQuery({ ...headInfoQueryOptions(repoProjectId), enabled: local });
	const { isPending: isUpdating, mutate: integrate } = useWorkspaceIntegrateUpstream();
	const openProject = useOpenProject();

	const target = headInfo?.target ?? null;
	const updateBase = () => {
		const updates = (headInfo?.stacks ?? [])
			.map(stackBottomRelativeTo)
			.filter((relativeTo) => relativeTo != null)
			.map((relativeTo): BottomUpdate => ({ kind: "rebase", selector: relativeTo }));
		integrate({ projectId: repoProjectId, updates, dryRun: false });
	};
	const localBranches =
		(headInfo?.stacks ?? []).flatMap((stack) => stack.segments).filter((segment) => segment.refName)
			.length + (headInfo?.worktrees.length ?? 0);
	const others = published.toSorted((a, b) =>
		a.name === machine ? -1 : b.name === machine ? 1 : 0,
	);

	return (
		<>
			<header className={styles.header}>
				<div className={styles.titleRow}>
					<h2 className={classes("text-15", "text-semibold")}>{project?.title ?? repoProjectId}</h2>
					{local && repoProjectId !== projectId && (
						<Button size="small" onClick={() => openProject(repoProjectId)}>
							Open
						</Button>
					)}
				</div>
				{local && project !== undefined && (
					<span className={classes("text-12", styles.muted)}>{project.path}</span>
				)}
			</header>

			{target !== null && (
				<Section heading="Base">
					<Item
						title={
							target.isCurrent
								? `Up to date with ${target.remoteTrackingRef.remoteName}/${target.remoteTrackingRef.displayName}`
								: `${count(target.commitsAhead, "commit", "commits")} behind ${target.remoteTrackingRef.remoteName}/${target.remoteTrackingRef.displayName}`
						}
						action={
							!target.isCurrent && (
								<Button size="small" disabled={isUpdating} onClick={updateBase}>
									Update base
								</Button>
							)
						}
					/>
				</Section>
			)}

			<Section heading="Machines">
				{local && (
					<Item
						title="This machine"
						detail={headInfo === undefined ? undefined : count(localBranches, "branch", "branches")}
					/>
				)}
				{others.map((other) => (
					<Item
						key={other.name}
						title={other.name}
						detail={
							<>
								{online.includes(other.name) ? "Online" : "Offline"}
								{" · "}
								{count(other.branches.length, "branch", "branches")}
								{" · published "}
								<RelativeTime timestamp={other.publishedAt} />
							</>
						}
					/>
				))}
				{!local && others.length === 0 && <Item title="No machine published this yet" />}
			</Section>

			{(headInfo?.worktrees.length ?? 0) > 0 && (
				<Section heading="Worktrees">
					{headInfo?.worktrees.map((worktree) => (
						<Item
							key={worktree.name}
							title={`${worktree.name}/`}
							detail={worktree.refName?.displayName ?? "Detached"}
						/>
					))}
				</Section>
			)}
		</>
	);
};
