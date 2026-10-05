import { encodeBytes } from "#ui/api/bytes.ts";
import {
	changesInWorktreeQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	hostedProjectQueryOptions,
	hostedPresenceQueryOptions,
	listProjectsQueryOptions,
	worktreeChangesQueryOptions,
	hostedAccountQueryOptions,
} from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import type {
	Commit,
	HostedAccountProject,
	HostedProject,
	PublishState,
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
	/** How this machine's branch compares with what it last published; null if never published. */
	publishState: PublishState | null;
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
	/** How many branches another machine published, known before they're loaded. */
	branchCount: number | null;
	/** Published by another machine to a repo this machine has no checkout of. */
	remoteOnly: boolean;
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

const byName = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base" });

/**
 * Newest activity first, so the mesh reads as a feed; by name where neither has any. Rows that
 * move as unfolding loads their activity stay put while the pointer is over the tree.
 */
const byActivity =
	<T extends { at: number | null }>(name: (item: T) => string) =>
	(a: T, b: T) =>
		(b.at ?? -Infinity) - (a.at ?? -Infinity) || byName(name(a), name(b));

const thisMachineFirst =
	<T extends { isThisMachine: boolean; at: number | null }>(name: (item: T) => string) =>
	(a: T, b: T) =>
		a.isThisMachine === b.isThisMachine ? byActivity(name)(a, b) : a.isThisMachine ? -1 : 1;

const localBranches = (
	segments: Array<Segment>,
	published: ReadonlyMap<string, PublishState>,
	worktree?: string,
): Array<MeshBranch> =>
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
						publishState: published.get(segment.refName.displayName) ?? null,
					},
				],
	);

/** Stands in for a project id where this machine has no checkout, so the repo has no project. */
const remoteOnlyId = (root: string) => `hub:${root}`;

export const isRemoteOnlyId = (projectId: string) => projectId.startsWith("hub:");

/** Where `worktreeFiles` keeps a worktree's files. */
const filesKey = (projectId: string, worktree: string) => `${projectId}\0${worktree}`;

const buildTree = (
	projects: Array<ProjectForFrontend>,
	heads: Array<UseQueryResult<RefInfo>>,
	changes: Array<UseQueryResult<WorktreeChanges>>,
	hosted: Array<UseQueryResult<HostedProject>>,
	presence: Array<UseQueryResult<Array<string>>>,
	worktreeFiles: ReadonlyMap<string, Array<string>>,
	account: Array<HostedAccountProject>,
): MeshTree => {
	// A hosted page runs on no machine: its projects are the server's copies of what machines sent.
	const onServer = window.lite.hosted === true;
	const online = new Set(presence.flatMap((result) => result.data ?? []));
	const checkouts: Array<MeshCheckout> = [];
	const accountByProject = new Map(
		account.flatMap((entry) =>
			(entry.projectIds ?? []).map((projectId) => [projectId, entry] as const),
		),
	);
	// Repos of the same name read by their parent directory too, e.g. `work/api` and `forks/api`.
	const titleCount = Map.groupBy(projects, (project) => project.title);
	const repoName = (project: ProjectForFrontend) =>
		(titleCount.get(project.title)?.length ?? 0) > 1
			? `${project.path.split("/").filter(Boolean).at(-2) ?? ""}/${project.title}`
			: project.title;
	projects.forEach((project, i) => {
		if (!onServer) {
			const head = heads[i]?.data;
			const published = new Map(
				(hosted[i]?.data?.publishedHere ?? []).map(({ branch, state }) => [branch, state]),
			);
			const branches = localBranches(
				head?.stacks.flatMap((stack) => stack.segments) ?? [],
				published,
			);
			const worktrees = (head?.worktrees ?? []).map((worktree) => ({
				name: worktree.name,
				branches: localBranches(worktree.segments, published, worktree.name),
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
				branchCount: null,
				remoteOnly: false,
				branches,
				worktrees,
				at: newest(
					[...branches, ...worktrees.flatMap((worktree) => worktree.branches)].map(
						(branch) => branch.at,
					),
				),
			});
		}
		// Branches once loaded; until then, what the account listing says each machine published.
		const loaded = hosted[i]?.data?.machines;
		const listed = accountByProject.get(project.id)?.machines ?? [];
		for (const summary of loaded === undefined ? listed : []) {
			checkouts.push({
				projectId: project.id,
				repo: repoName(project),
				machine: summary.name,
				isThisMachine: false,
				online: online.has(summary.name),
				uncommittedFiles: 0,
				branchCount: summary.branches,
				remoteOnly: false,
				branches: [],
				worktrees: [],
				at: summary.publishedAt,
			});
		}
		for (const machine of loaded ?? []) {
			checkouts.push({
				projectId: project.id,
				repo: repoName(project),
				machine: machine.name,
				isThisMachine: false,
				online: online.has(machine.name),
				uncommittedFiles: 0,
				branchCount: machine.branches.length,
				remoteOnly: false,
				branches: machine.branches.map((branch) => ({
					name: branch.branch,
					ref: encodeBytes(branch.refName),
					commits: branch.commits,
					at: machine.publishedAt,
					uncommitted: branch.uncommitted,
					sent: branch.sent,
					publishState: null,
				})),
				worktrees: [],
				at: machine.publishedAt,
			});
		}
	});

	for (const entry of account) {
		if ((entry.projectIds ?? []).length > 0) continue;
		for (const summary of entry.machines) {
			checkouts.push({
				projectId: remoteOnlyId(entry.root),
				repo: entry.title,
				machine: summary.name,
				isThisMachine: false,
				online: online.has(summary.name),
				uncommittedFiles: 0,
				branchCount: summary.branches,
				remoteOnly: true,
				branches: [],
				worktrees: [],
				at: summary.publishedAt,
			});
		}
	}

	const byMachine = Map.groupBy(checkouts, (checkout) => checkout.machine);
	// A machine that's online is listed whether or not it published anything yet.
	for (const name of online) if (!byMachine.has(name)) byMachine.set(name, []);

	const machines: Array<MeshMachine> = [...byMachine].map(([name, list]) => ({
		name,
		isThisMachine: name === THIS_MACHINE,
		online: name === THIS_MACHINE || online.has(name),
		at: newest(list.map((c) => c.at)),
		checkouts: list.toSorted(byActivity((checkout) => checkout.repo)),
	}));
	machines.sort(thisMachineFirst((machine) => machine.name));

	const repos: Array<MeshRepo> = [...Map.groupBy(checkouts, (c) => c.projectId)].map(
		([projectId, list]) => ({
			projectId,
			name: list[0]?.repo ?? projectId,
			path: list.find((checkout) => checkout.path !== undefined)?.path,
			at: newest(list.map((c) => c.at)),
			checkouts: list.toSorted(thisMachineFirst((checkout) => checkout.machine)),
		}),
	);
	repos.sort(byActivity((repo) => repo.name));

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
	const local = window.lite.hosted !== true;
	const { data: hostedEnabled } = useQuery({
		...guiSettingsQueryOptions,
		// As the workspace's remote machines: the hosted page shows them whatever the setting says.
		select: (settings) =>
			window.lite.hosted === true || (settings.hostedBranches ?? defaultSettings.hostedBranches),
	});
	// Grouped by repo, an unfolded repo may show this machine's branches without a row of its own.
	const localUnfolded = (projectId: string) =>
		unfolded[checkoutKey(THIS_MACHINE, projectId)] === true ||
		(grouping === "repos" && unfolded[repoKey(projectId)] === true);

	// The hosted page's projects are the server's own, where each listing is cheap; a machine asks
	// the server once for everything, and a project only for a repo unfolded under a machine.
	const { data: account = [] } = useQuery({
		...hostedAccountQueryOptions,
		enabled: hostedEnabled === true && local,
	});
	const remoteUnfolded = (projectId: string) =>
		Object.keys(unfolded).some(
			(key) =>
				key.startsWith("checkout:") &&
				key.endsWith(`:${projectId}`) &&
				!key.startsWith(checkoutKey(THIS_MACHINE, "")),
		);
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
				...hostedProjectQueryOptions(project.id),
				enabled:
					hostedEnabled === true &&
					(local
						? // This machine's own unfolded repo too, for how its branches compare with
							// what it published.
							localUnfolded(project.id) ||
							unfolded[repoKey(project.id)] === true ||
							remoteUnfolded(project.id)
						: grouping === "machines" || unfolded[repoKey(project.id)] === true),
			})),
			hostedPresenceQueryOptions,
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
				results.slice(2 * count, 3 * count) as Array<UseQueryResult<HostedProject>>,
				results.slice(3 * count, 3 * count + 1) as Array<UseQueryResult<Array<string>>>,
				new Map(
					worktreesUnfolded.map(({ projectId, worktree }, i) => [
						filesKey(projectId, worktree),
						(results[3 * count + 1 + i]?.data as WorktreeChanges | undefined)?.changes.map(
							(change) => change.path,
						) ?? [],
					]),
				),
				account,
			);
		},
	});
};
