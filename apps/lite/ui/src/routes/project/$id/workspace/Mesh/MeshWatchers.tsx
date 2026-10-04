import { useQueryClient } from "@tanstack/react-query";
import { type FC, useEffect } from "react";
import type { QueryKeyPrefix } from "#ui/api/query-keys.ts";
import type { MeshGrouping } from "#ui/interface/state.ts";
import { handleProjectEvent } from "#ui/project-events.ts";
import { checkoutKey, repoKey } from "./mesh-rows.ts";
import { THIS_MACHINE } from "./useMeshTree.ts";

/** Each watcher scans its worktree and recomputes status on change, so only so many run. */
const MAX_WATCHED = 10;

/**
 * Keeps a project watcher on each local repo unfolded in the mesh, the most recently unfolded
 * first, so what it shows of them stays live as the open project's does. The open project is
 * watched already.
 */
export const MeshWatchers: FC<{
	projectId: string;
	grouping: MeshGrouping;
	unfolded: Record<string, true>;
}> = ({ projectId, grouping, unfolded }) => {
	if (window.lite.hosted === true) return null;
	const prefixes = [
		`${checkoutKey(THIS_MACHINE, "")}`,
		...(grouping === "repos" ? [repoKey("")] : []),
	];
	// Keys keep the order they were unfolded in, so the last ones are the most recent.
	const watched = [
		...new Set(
			Object.keys(unfolded)
				.toReversed()
				.flatMap((key) => {
					const prefix = prefixes.find((candidate) => key.startsWith(candidate));
					const id = prefix === undefined ? "" : key.slice(prefix.length);
					// Only the repo's own key: a worktree or branch under it has more after the id.
					return id === "" || id.includes(":") || id === projectId ? [] : [id];
				}),
		),
	].slice(0, MAX_WATCHED);
	return watched.map((id) => <ProjectWatcher key={id} projectId={id} />);
};

const ProjectWatcher: FC<{ projectId: string }> = ({ projectId }) => {
	const queryClient = useQueryClient();
	// The watcher lives outside React, held while this row is unfolded.
	useEffect(() => {
		const subscription = window.lite
			.watcherSubscribe(projectId, (event) => handleProjectEvent(event, projectId, queryClient))
			.then((id) => {
				// Whatever changed before it was watched.
				void queryClient.invalidateQueries<QueryKeyPrefix>({ queryKey: [projectId] });
				return id;
			})
			.catch(() => undefined);
		return () => {
			void subscription.then(
				(id) => id !== undefined && window.lite.watcherUnsubscribe(id).catch(() => false),
			);
		};
	}, [projectId, queryClient]);
	return null;
};
