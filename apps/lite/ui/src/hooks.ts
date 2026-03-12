import { LongRunningTaskSnapshot } from "#electron/ipc";
import { QueryClient, useMutation, useQuery } from "@tanstack/react-query";
import { useEffect } from "react";

const LONG_RUNNING_TASKS_QUERY_KEY = ["long-running-tasks"] as const;

/**
 * List the tasks and subscribe to updates of it.
 */
export function useTasks(queryClient: QueryClient) {
	const tasksQuery = useQuery({
		queryKey: LONG_RUNNING_TASKS_QUERY_KEY,
		queryFn: async (): Promise<LongRunningTaskSnapshot[]> => {
			return await window.lite.listLongRunningTasks();
		},
		initialData: [],
	});

	useEffect(() => {
    // This unsubscribes on unmount
		return window.lite.onLongRunningTaskEvent((event) => {
			queryClient.setQueryData(
				LONG_RUNNING_TASKS_QUERY_KEY,
				(currentTasks: LongRunningTaskSnapshot[] = []) => {
					const hasTask = currentTasks.some((task) => task.taskId === event.taskId);
					if (!hasTask) {
						return sortTasksByIdDesc([event, ...currentTasks]);
					}

					return sortTasksByIdDesc(
						currentTasks.map((task) => {
							if (task.taskId !== event.taskId) {
								return task;
							}

							return event;
						})
					);
				}
			);
		});
	}, [queryClient]);

	return tasksQuery;
}


/**
 * Hook for starting and cancelling tasks.
 */
export function useTaskMutations() {
	const startTaskMutation = useMutation({
		mutationFn: async (durationMs: number): Promise<number> => {
			return await window.lite.startLongRunningTask(durationMs);
		},
	});

	const cancelTaskMutation = useMutation({
		mutationFn: async (taskId: number): Promise<boolean> => {
			return await window.lite.cancelLongRunningTask(taskId);
		},
	});
	return { startTaskMutation, cancelTaskMutation };
}


function sortTasksByIdDesc(tasks: LongRunningTaskSnapshot[]): LongRunningTaskSnapshot[] {
	return [...tasks].sort((left, right) => right.taskId - left.taskId);
}