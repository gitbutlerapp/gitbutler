import { Toast, type ToastManager, Tooltip } from "@base-ui/react";
import { useWorkerPool, WorkerPoolContextProvider } from "@pierre/diffs/react";
import { type QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { type RegisteredRouter, RouterProvider } from "@tanstack/react-router";
import { type FC, StrictMode, useEffect } from "react";
import { Provider } from "react-redux";
import { store } from "#ui/store.ts";
import { Toasts } from "#ui/components/Toasts.tsx";
import { AskpassPromptDialog } from "#ui/AskpassPromptDialog.tsx";
import { guiSettingsQueryOptions } from "./api/queries.ts";
import { defaultSettings } from "./settings.ts";

const workerFactory = (): Worker =>
	new Worker(new URL("@pierre/diffs/worker/worker.js", import.meta.url), {
		type: "module",
	});

// Must be mounted under the worker pool provider.
const SyntaxThemeSync: FC = () => {
	const workerPool = useWorkerPool();
	const { data: theme } = useQuery({
		...guiSettingsQueryOptions,
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

// The stylesheet reads the choice off the root element (see control-cursor.css).
const HandCursorSync: FC = () => {
	const { data: handCursor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.handCursor ?? defaultSettings.handCursor,
	});

	useEffect(() => {
		document.documentElement.toggleAttribute("data-hand-cursor", handCursor === true);
	}, [handCursor]);

	return null;
};

export const App: FC<{
	queryClient: QueryClient;
	toastManager: ToastManager;
	router: RegisteredRouter;
}> = ({ queryClient, toastManager, router }) => (
	<StrictMode>
		<Provider store={store}>
			<QueryClientProvider client={queryClient}>
				<Toast.Provider toastManager={toastManager}>
					<Tooltip.Provider>
						<WorkerPoolContextProvider
							poolOptions={{ workerFactory }}
							highlighterOptions={{ preferredHighlighter: "shiki-wasm" }}
						>
							<SyntaxThemeSync />
							<HandCursorSync />
							<RouterProvider router={router} />
							<AskpassPromptDialog />
							<Toasts />
						</WorkerPoolContextProvider>
					</Tooltip.Provider>
				</Toast.Provider>
				<ReactQueryDevtools />
			</QueryClientProvider>
		</Provider>
	</StrictMode>
);
