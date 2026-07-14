import { Toast, ToastManager, Tooltip } from "@base-ui/react";
import { useWorkerPool, WorkerPoolContextProvider } from "@pierre/diffs/react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RegisteredRouter, RouterProvider } from "@tanstack/react-router";
import { type FC, useEffect, useState } from "react";
import { Provider } from "react-redux";
import { CheckedCommitIdsRegistryContext } from "#ui/CheckedCommitIdsContext.ts";
import { store } from "#ui/store.ts";
import { Toasts } from "#ui/components/Toasts.tsx";
import { DetailsFullWindowContext } from "#ui/DetailsFullWindowContext.ts";
import { DialogContext } from "#ui/DialogContext.ts";
import { FilesVisibleRegistryContext } from "#ui/FilesVisibleContext.ts";
import type { Dialog } from "#ui/projects/project.ts";
import { AskpassPromptDialog } from "#ui/AskpassPromptDialog.tsx";
import { getGUISettingsQueryOptions } from "./api/queries.ts";
import { defaultSettings } from "./settings.ts";

const workerFactory = (): Worker =>
	new Worker(new URL("@pierre/diffs/worker/worker.js", import.meta.url), {
		type: "module",
	});

// Must be mounted under the worker pool provider.
const ThemeSync: FC = () => {
	const workerPool = useWorkerPool();
	const { data: theme } = useQuery({
		...getGUISettingsQueryOptions(),
		select: (cfg) => cfg.syntaxHighlighting,
	});

	useEffect(() => {
		void workerPool?.setRenderOptions({
			theme: {
				light: theme?.light ?? defaultSettings.syntaxHighlighting.light,
				dark: theme?.dark ?? defaultSettings.syntaxHighlighting.dark,
			},
		});
	}, [workerPool, theme]);

	return null;
};

// Reuse for reference stability.
const emptySet: Set<never> = new Set();

export const App: FC<{
	queryClient: QueryClient;
	toastManager: ToastManager;
	router: RegisteredRouter;
}> = ({ queryClient, toastManager, router }) => {
	const [detailsFullWindow, setDetailsFullWindow] = useState(false);
	const toggleDetailsFullWindow = () => setDetailsFullWindow((fullWindow) => !fullWindow);

	const [filesVisibleByProjectId, setFilesVisibleByProjectId] = useState(() => new Set<string>());
	const filesVisibleContext = (projectId: string) => ({
		filesVisible: filesVisibleByProjectId.has(projectId),
		toggleFiles: () =>
			setFilesVisibleByProjectId((curr) => {
				const next = new Set(curr);
				if (next.has(projectId)) next.delete(projectId);
				else next.add(projectId);
				return next;
			}),
	});

	const [checkedCommitIdsByProjectId, setCheckedCommitIdsByProjectId] = useState(
		() => new Map<string, Set<string>>(),
	);
	const mapCheckedCommitIdsBy = (
		projectId: string,
		update: (current: Set<string>) => Set<string>,
	) =>
		setCheckedCommitIdsByProjectId((curr) => {
			const current = curr.get(projectId) ?? emptySet;
			const next = update(current);
			return next === current ? curr : new Map(curr).set(projectId, next);
		});
	const checkedCommitIdsContext = (projectId: string) => ({
		checkedCommitIds: checkedCommitIdsByProjectId.get(projectId) ?? emptySet,
		setCommitsChecked: (commitIds: Array<string>, checked: boolean) =>
			mapCheckedCommitIdsBy(projectId, (curr) => {
				const toggled = new Set(commitIds);
				return checked ? curr.union(toggled) : curr.difference(toggled);
			}),
		clearCheckedCommits: () => mapCheckedCommitIdsBy(projectId, () => emptySet),
		updateRewrittenCommitReferences: (replacedCommits: Record<string, string>) =>
			mapCheckedCommitIdsBy(projectId, (curr) => {
				let next: Set<string> | undefined;
				for (const id of curr) {
					const newId = replacedCommits[id];
					if (newId === undefined || newId === id) continue;

					next ??= new Set(curr);
					next.delete(id);
					next.add(newId);
				}
				return next ?? curr;
			}),
	});

	const [dialog, setDialog] = useState<Dialog>({ _tag: "None" });
	const openDialog = (nextDialog: Dialog) => setDialog(nextDialog);
	const closeDialog = () => setDialog({ _tag: "None" });

	return (
		<CheckedCommitIdsRegistryContext value={checkedCommitIdsContext}>
			<FilesVisibleRegistryContext value={filesVisibleContext}>
				<DetailsFullWindowContext
					value={{ detailsFullWindow, setDetailsFullWindow, toggleDetailsFullWindow }}
				>
					<DialogContext value={{ dialog, openDialog, closeDialog }}>
						<Provider store={store}>
							<QueryClientProvider client={queryClient}>
								<Toast.Provider toastManager={toastManager}>
									<Tooltip.Provider>
										<WorkerPoolContextProvider
											poolOptions={{ workerFactory }}
											highlighterOptions={{ preferredHighlighter: "shiki-wasm" }}
										>
											<ThemeSync />
											<RouterProvider router={router} />
											<AskpassPromptDialog />
											<Toasts />
										</WorkerPoolContextProvider>
									</Tooltip.Provider>
								</Toast.Provider>
								<ReactQueryDevtools />
							</QueryClientProvider>
						</Provider>
					</DialogContext>
				</DetailsFullWindowContext>
			</FilesVisibleRegistryContext>
		</CheckedCommitIdsRegistryContext>
	);
};
