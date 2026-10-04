import { hostedBranchRef } from "#ui/branch.ts";
import {
	changesInWorktreeQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	hostedMachinesQueryOptions,
	hostedPresenceQueryOptions,
	listProjectsQueryOptions,
	worktreeChangesQueryOptions,
} from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import type {
	Commit,
	HostedMachine,
	ProjectForFrontend,
	RefInfo,
	Segment,
	WorktreeChanges,
} from "@gitbutler/but-sdk";
import { type UseQueryResult, useQueries, useQuery } from "@tanstack/react-query";
import type { MeshGrouping } from "#ui/interface/state.ts";
import { checkoutKey, repoKey, worktreeKey } from "./mesh-rows.ts";

export type MeshBranch = {
	name: string;
	/** The full ref it's selected by: a local branch's own, or the fetched copy of another machine's. */
	ref: Array<number>;
	/** The linked worktree it's checked out in, whose commits are addressed as its own. */
	worktree?: string;
	commits: Array<Commit>;
	/** Its newest commit, or when the machine published it. */
	at: number | null;
	/** What another machine published of its uncommitted changes, as a commit on top. */
	uncommitted: Commit | null;
	/** Another machine sent it to this one, to pull or dismiss. */
	sent: boolean;
};

/** One repository as one machine has it: a local project here, or what another machine published. */
export type MeshCheckout = {
	projectId: string;
	repo: string;
	/** Where this machine has it; another machine's isn't known. */
	path?: string;
	machine: string;
	isThisMachine: boolean;
	online: boolean;
	/** Files changed in this machine's checkout; another machine's aren't known. */
	uncommittedFiles: number;
	/** In the workspace, or published by another machine. */
	branches: Array<MeshBranch>;
	/** This machine's linked worktrees, each with its own branch and files. */
	worktrees: Array<MeshWorktree>;
	at: number | null;
};

export type MeshWorktree = {
	name: string;
	branches: Array<MeshBranch>;
	/** Its uncommitted files, once unfolded and loaded. */
	files: Array<string>;
};

export type MeshMachine = {
	name: string;
	isThisMachine: boolean;
	online: boolean;
	at: number | null;
	checkouts: Array<MeshCheckout>;
};

export type MeshRepo = {
	projectId: string;
	name: string;
	path?: string;
	at: number | null;
	checkouts: Array<MeshCheckout>;
};

type MeshTree = {
	machines: Array<MeshMachine>;
	repos: Array<MeshRepo>;
};

/**
 * This machine has no name the UI knows; the hosted server names it by host name. Published
 * names have no spaces, so this never collides with one.
 */
export const THIS_MACHINE = "This machine";

const newest = (times: Array<number | null>): number | null =>
	times.reduce<number | null>(
		(max, t) => (t !== null && (max === null || t > max) ? t : max),
		null,
	);

const byNewest = <T extends { at: number | null }>(a: T, b: T) => (b.at ?? 0) - (a.at ?? 0);

const thisMachineFirst = <T extends { at: number | null; isThisMachine: boolean }>(a: T, b: T) =>
	a.isThisMachine === b.isThisMachine ? byNewest(a, b) : a.isThisMachine ? -1 : 1;

const localBranches = (segments: Array<Segment>, worktree?: string): Array<MeshBranch> =>
	segments.flatMap((segment) =>
		segment.refName === null
			? []
			: [
					{
						name: segment.refName.displayName,
						ref: segment.refName.fullNameBytes,
						worktree,
						commits: segment.commits,
						at: newest(segment.commits.map((commit) => commit.committedAt)),
						uncommitted: null,
						sent: false,
					},
				],
	);

/** Where `worktreeFiles` keeps a worktree's files. */
const filesKey = (projectId: string, worktree: string) => `${projectId}\0${worktree}`;

const buildTree = (
	projects: Array<ProjectForFrontend>,
	heads: Array<UseQueryResult<RefInfo>>,
	changes: Array<UseQueryResult<WorktreeChanges>>,
	hosted: Array<UseQueryResult<Array<HostedMachine>>>,
	presence: Array<UseQueryResult<Array<string>>>,
	worktreeFiles: ReadonlyMap<string, Array<string>>,
): MeshTree => {
	// A hosted page runs on no machine: its projects are the server's copies of what machines sent.
	const onServer = window.lite.hosted === true;
	// Presence is the account's, so any project's listener tells it.
	const online = new Set(presence.flatMap((result) => result.data ?? []));
	const checkouts: Array<MeshCheckout> = [];
	// Repos of the same name read by their parent directory too, e.g. `work/api` and `forks/api`.
	const titleCount = Map.groupBy(projects, (project) => project.title);
	const repoName = (project: ProjectForFrontend) =>
		(titleCount.get(project.title)?.length ?? 0) > 1
			? `${project.path.split("/").filter(Boolean).at(-2) ?? ""}/${project.title}`
			: project.title;
	projects.forEach((project, i) => {
		if (!onServer) {
			const head = heads[i]?.data;
			const branches = localBranches(head?.stacks.flatMap((stack) => stack.segments) ?? []);
			const worktrees = (head?.worktrees ?? []).map((worktree) => ({
				name: worktree.name,
				branches: localBranches(worktree.segments, worktree.name),
				files: worktreeFiles.get(filesKey(project.id, worktree.name)) ?? [],
			}));
			checkouts.push({
				projectId: project.id,
				repo: repoName(project),
				path: window.lite.hosted === true ? undefined : project.path,
				machine: THIS_MACHINE,
				isThisMachine: true,
				online: true,
				uncommittedFiles: changes[i]?.data?.changes.length ?? 0,
				branches,
				worktrees,
				at: newest(
					[...branches, ...worktrees.flatMap((worktree) => worktree.branches)].map(
						(branch) => branch.at,
					),
				),
			});
		}
		for (const machine of hosted[i]?.data ?? []) {
			checkouts.push({
				projectId: project.id,
				repo: repoName(project),
				machine: machine.name,
				isThisMachine: false,
				online: online.has(machine.name),
				uncommittedFiles: 0,
				branches: machine.branches.map((branch) => ({
					name: branch.branch,
					ref: hostedBranchRef(machine.name, branch.branch),
					commits: branch.commits,
					at: machine.publishedAt,
					uncommitted: branch.uncommitted,
					sent: branch.sent,
				})),
				worktrees: [],
				at: machine.publishedAt,
			});
		}
	});

	const byMachine = Map.groupBy(checkouts, (checkout) => checkout.machine);
	// A machine that's online is listed whether or not it published anything yet.
	for (const name of online) if (!byMachine.has(name)) byMachine.set(name, []);

	const machines: Array<MeshMachine> = [...byMachine].map(([name, list]) => ({
		name,
		isThisMachine: name === THIS_MACHINE,
		online: name === THIS_MACHINE || online.has(name),
		at: newest(list.map((c) => c.at)),
		checkouts: list.toSorted(byNewest),
	}));
	machines.sort(thisMachineFirst);

	const repos: Array<MeshRepo> = [...Map.groupBy(checkouts, (c) => c.projectId)].map(
		([projectId, list]) => ({
			projectId,
			name: list[0]?.repo ?? projectId,
			path: list.find((checkout) => checkout.path !== undefined)?.path,
			at: newest(list.map((c) => c.at)),
			checkouts: list.toSorted(thisMachineFirst),
		}),
	);
	repos.sort(byNewest);

	return { machines, repos };
};

/**
 * Every local project and every machine that published one, grouped by machine and by repo.
 * Built in `combine`, so the tree keeps its identity until the data behind it changes.
 *
 * A project's branches load once its checkout here, or grouped by repo its repo, is unfolded. What other machines published
 * loads once its repo is unfolded, except grouped by machine, where it's how machines are found.
 */
export const useMeshTree = ({
	grouping,
	unfolded,
}: {
	grouping: MeshGrouping;
	unfolded: Record<string, true>;
}): MeshTree => {
	const { data: projects = [] } = useQuery(listProjectsQueryOptions);
	const { data: hostedEnabled } = useQuery({
		...guiSettingsQueryOptions,
		// As the workspace's remote machines: the hosted page shows them whatever the setting says.
		select: (settings) =>
			window.lite.hosted === true || (settings.hostedBranches ?? defaultSettings.hostedBranches),
	});
	const local = window.lite.hosted !== true;
	// Grouped by repo, an unfolded repo may show this machine's branches without a row of its own.
	const localUnfolded = (projectId: string) =>
		unfolded[checkoutKey(THIS_MACHINE, projectId)] === true ||
		(grouping === "repos" && unfolded[repoKey(projectId)] === true);

	// An unfolded worktree's files; its key names the project and worktree, so no head is needed.
	const worktreesUnfolded = projects.flatMap((project) => {
		const prefix = worktreeKey(checkoutKey(THIS_MACHINE, project.id), "");
		return Object.keys(unfolded).flatMap((key) =>
			key.startsWith(prefix) ? [{ projectId: project.id, worktree: key.slice(prefix.length) }] : [],
		);
	});

	return useQueries({
		queries: [
			...projects.map((project) => ({
				...headInfoQueryOptions(project.id),
				enabled: local && localUnfolded(project.id),
			})),
			...projects.map((project) => ({
				...changesInWorktreeQueryOptions(project.id),
				enabled: local && localUnfolded(project.id),
			})),
			...projects.map((project) => ({
				...hostedMachinesQueryOptions(project.id),
				enabled:
					hostedEnabled === true &&
					(grouping === "machines" || unfolded[repoKey(project.id)] === true),
			})),
			...projects.map((project) => hostedPresenceQueryOptions(project.id)),
			...worktreesUnfolded.map(({ projectId, worktree }) => ({
				...worktreeChangesQueryOptions(projectId, worktree),
				enabled: local,
			})),
		],
		combine: (results) => {
			const count = projects.length;
			return buildTree(
				projects,
				results.slice(0, count) as Array<UseQueryResult<RefInfo>>,
				results.slice(count, 2 * count) as Array<UseQueryResult<WorktreeChanges>>,
				results.slice(2 * count, 3 * count) as Array<UseQueryResult<Array<HostedMachine>>>,
				results.slice(3 * count, 4 * count) as Array<UseQueryResult<Array<string>>>,
				new Map(
					worktreesUnfolded.map(({ projectId, worktree }, i) => [
						filesKey(projectId, worktree),
						(results[4 * count + i]?.data as WorktreeChanges | undefined)?.changes.map(
							(change) => change.path,
						) ?? [],
					]),
				),
			);
		},
	});
};
