import { Toast, ToastManager, Tooltip } from "@base-ui/react";
import { useWorkerPool, WorkerPoolContextProvider } from "@pierre/diffs/react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RegisteredRouter, RouterProvider } from "@tanstack/react-router";
import { type FC, useEffect, useState } from "react";
import { CheckedCommitIdsRegistryContext } from "#ui/CheckedCommitIdsContext.ts";
import { CommitTargetRegistryContext } from "#ui/CommitTargetContext.ts";
import { Toasts } from "#ui/components/Toasts.tsx";
import { DetailsFullWindowContext } from "#ui/DetailsFullWindowContext.ts";
import { DialogContext, type Dialog } from "#ui/DialogContext.ts";
import { FilesVisibleRegistryContext } from "#ui/FilesVisibleContext.ts";
import { HighlightedCommitIdsRegistryContext } from "#ui/HighlightedCommitIdsContext.ts";
import { useProjectRegistry } from "#ui/ProjectRegistry.ts";
import { WorkspaceRegistryContext } from "#ui/WorkspaceContext.ts";
import { createWorkspace } from "#ui/workspace.ts";
import { AskpassPromptDialog } from "#ui/AskpassPromptDialog.tsx";
import { getGUISettingsQueryOptions } from "./api/queries.ts";
import { defaultSettings } from "./settings.ts";
import type { RelativeTo } from "@gitbutler/but-sdk";

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

export const App: FC<{
	queryClient: QueryClient;
	toastManager: ToastManager;
	router: RegisteredRouter;
}> = ({ queryClient, toastManager, router }) => {
	const [detailsFullWindow, setDetailsFullWindow] = useState(false);
	const toggleDetailsFullWindow = () => setDetailsFullWindow((fullWindow) => !fullWindow);

	const [filesVisibleByProjectId, setFilesVisibleByProjectId] = useState(() => new Set<string>());
	const filesVisibleContext = (currentProjectId: string) => ({
		filesVisible: filesVisibleByProjectId.has(currentProjectId),
		toggleFiles: (projectId: string) =>
			setFilesVisibleByProjectId((curr) => {
				const next = new Set(curr);
				if (next.has(projectId)) next.delete(projectId);
				else next.add(projectId);
				return next;
			}),
	});

	const checkedCommitIdsRegistry = useProjectRegistry(new Set<string>());
	const highlightedCommitIdsRegistry = useProjectRegistry(new Set<string>());
	const commitTargetRegistry = useProjectRegistry<RelativeTo | null>(null);
	const workspaceRegistry = useProjectRegistry(createWorkspace());

	const [dialog, setDialog] = useState<Dialog>({ _tag: "None" });
	const openDialog = (nextDialog: Dialog) => setDialog(nextDialog);
	const closeDialog = () => setDialog({ _tag: "None" });

	return (
		<WorkspaceRegistryContext value={workspaceRegistry}>
			<CommitTargetRegistryContext value={commitTargetRegistry}>
				<HighlightedCommitIdsRegistryContext value={highlightedCommitIdsRegistry}>
					<CheckedCommitIdsRegistryContext value={checkedCommitIdsRegistry}>
						<FilesVisibleRegistryContext value={filesVisibleContext}>
							<DetailsFullWindowContext
								value={{ detailsFullWindow, setDetailsFullWindow, toggleDetailsFullWindow }}
							>
								<DialogContext value={{ dialog, openDialog, closeDialog }}>
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
								</DialogContext>
							</DetailsFullWindowContext>
						</FilesVisibleRegistryContext>
					</CheckedCommitIdsRegistryContext>
				</HighlightedCommitIdsRegistryContext>
			</CommitTargetRegistryContext>
		</WorkspaceRegistryContext>
	);
};
