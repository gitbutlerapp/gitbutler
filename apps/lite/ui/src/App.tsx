import { Toast, ToastManager, Tooltip } from "@base-ui/react";
import { useWorkerPool, WorkerPoolContextProvider } from "@pierre/diffs/react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RegisteredRouter, RouterProvider } from "@tanstack/react-router";
import { type FC, useEffect, useState } from "react";
import { Provider } from "react-redux";
import { store } from "#ui/store.ts";
import { Toasts } from "#ui/components/Toasts.tsx";
import { DetailsFullWindowContext } from "#ui/DetailsFullWindowContext.ts";
import { DialogContext } from "#ui/DialogContext.ts";
import { FilesVisibleContext } from "#ui/FilesVisibleContext.ts";
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

export const App: FC<{
	queryClient: QueryClient;
	toastManager: ToastManager;
	router: RegisteredRouter;
}> = ({ queryClient, toastManager, router }) => {
	const [detailsFullWindow, setDetailsFullWindow] = useState(false);
	const [filesVisible, setFilesVisible] = useState(false);
	const [dialog, setDialog] = useState<Dialog>({ _tag: "None" });

	const toggleDetailsFullWindow = () => setDetailsFullWindow((fullWindow) => !fullWindow);
	const toggleFiles = () => setFilesVisible((visible) => !visible);
	const openDialog = (nextDialog: Dialog) => setDialog(nextDialog);
	const closeDialog = () => setDialog({ _tag: "None" });

	return (
		<FilesVisibleContext value={{ filesVisible, toggleFiles }}>
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
		</FilesVisibleContext>
	);
};
