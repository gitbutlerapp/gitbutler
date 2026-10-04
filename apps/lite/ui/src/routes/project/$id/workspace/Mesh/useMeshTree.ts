import {
	changesInWorktreeQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	hostedMachinesQueryOptions,
	hostedPresenceQueryOptions,
	listProjectsQueryOptions,
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
import { checkoutKey, repoKey } from "./mesh-rows.ts";

export type MeshBranch = {
	name: string;
	commits: Array<Commit>;
	/** Its newest commit, or when the machine published it. */
	at: number | null;
	/** What another machine published of its uncommitted changes, as a commit on top. */
	uncommitted: Commit | null;
};

/** One repository as one machine has it: a local project here, or what another machine published. */
export type MeshCheckout = {
	projectId: string;
	repo: string;
	machine: string;
	isThisMachine: boolean;
	online: boolean;
	/** Files changed in this machine's checkout; another machine's aren't known. */
	uncommittedFiles: number;
	branches: Array<MeshBranch>;
	at: number | null;
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
const THIS_MACHINE = "This machine";

const newest = (times: Array<number | null>): number | null =>
	times.reduce<number | null>(
		(max, t) => (t !== null && (max === null || t > max) ? t : max),
		null,
	);

const byNewest = <T extends { at: number | null }>(a: T, b: T) => (b.at ?? 0) - (a.at ?? 0);

const thisMachineFirst = <T extends { at: number | null; isThisMachine: boolean }>(a: T, b: T) =>
	a.isThisMachine === b.isThisMachine ? byNewest(a, b) : a.isThisMachine ? -1 : 1;

const localBranches = (headInfo: RefInfo | undefined): Array<MeshBranch> =>
	[...(headInfo?.stacks ?? []), ...(headInfo?.worktrees ?? [])]
		.flatMap((lane): Array<Segment> => lane.segments)
		.filter((segment) => segment.refName !== null)
		.map((segment) => ({
			name: segment.refName?.displayName ?? "",
			commits: segment.commits,
			at: newest(segment.commits.map((commit) => commit.committedAt)),
			uncommitted: null,
		}));

const buildTree = (
	projects: Array<ProjectForFrontend>,
	heads: Array<UseQueryResult<RefInfo>>,
	changes: Array<UseQueryResult<WorktreeChanges>>,
	hosted: Array<UseQueryResult<Array<HostedMachine>>>,
	presence: Array<UseQueryResult<Array<string>>>,
): MeshTree => {
	// A hosted page runs on no machine: its projects are the server's copies of what machines sent.
	const onServer = window.lite.hosted === true;
	// Presence is the account's, so any project's listener tells it.
	const online = new Set(presence.flatMap((result) => result.data ?? []));
	const checkouts: Array<MeshCheckout> = [];
	projects.forEach((project, i) => {
		if (!onServer) {
			const branches = localBranches(heads[i]?.data);
			checkouts.push({
				projectId: project.id,
				repo: project.title,
				machine: THIS_MACHINE,
				isThisMachine: true,
				online: true,
				uncommittedFiles: changes[i]?.data?.changes.length ?? 0,
				branches,
				at: newest(branches.map((branch) => branch.at)),
			});
		}
		for (const machine of hosted[i]?.data ?? []) {
			checkouts.push({
				projectId: project.id,
				repo: project.title,
				machine: machine.name,
				isThisMachine: false,
				online: online.has(machine.name),
				uncommittedFiles: 0,
				branches: machine.branches.map((branch) => ({
					name: branch.branch,
					commits: branch.commits,
					at: machine.publishedAt,
					uncommitted: branch.uncommitted,
				})),
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
		],
		combine: (results) => {
			const count = projects.length;
			return buildTree(
				projects,
				results.slice(0, count) as Array<UseQueryResult<RefInfo>>,
				results.slice(count, 2 * count) as Array<UseQueryResult<WorktreeChanges>>,
				results.slice(2 * count, 3 * count) as Array<UseQueryResult<Array<HostedMachine>>>,
				results.slice(3 * count) as Array<UseQueryResult<Array<string>>>,
			);
		},
	});
};
