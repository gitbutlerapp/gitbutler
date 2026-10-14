import {
	guiSettingsQueryOptions,
	workspaceFetchQueryOptions,
	workspaceFetchStatusQueryOptions,
} from "#ui/api/queries.ts";
import { addErrorToast } from "#ui/errors.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { Toast } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";

/**
 * Fetching from the remotes: the target's row has the button, the sidebar the
 * hotkey. Both call this; the query behind it is shared, so each sees the
 * other's fetch in flight.
 */
export const useFetchFromRemotes = (projectId: string) => {
	const toastManager = Toast.useToastManager();
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const { data: autoFetchFrequency } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.autoFetchFrequency,
	});
	const { data: status } = useQuery(workspaceFetchStatusQueryOptions(projectId));
	const { isFetching, refetch } = useQuery(
		workspaceFetchQueryOptions(projectId, autoFetchFrequency),
	);
	const fetch = () => {
		void refetch().then(({ error }) => {
			if (!error) return;

			addErrorToast(toastManager, {
				title: "Failed to fetch",
				error,
			});
		});
	};
	return {
		fetch,
		isPending: isFetching,
		enabled: noOperationPending && !isFetching,
		lastSuccessfulMs: status?.lastSuccessfulMs,
	};
};
