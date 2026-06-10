import { Toast, ToastManager, Tooltip } from "@base-ui/react";
import { WorkerPoolContextProvider } from "@pierre/diffs/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RegisteredRouter, RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { Provider } from "react-redux";
import { store } from "#ui/store.ts";
import { Toasts } from "#ui/components/Toasts.tsx";
import { Updater } from "#ui/Updater.tsx";
import { AskpassPromptDialog } from "#ui/AskpassPromptDialog.tsx";

const workerFactory = (): Worker =>
	new Worker(new URL("@pierre/diffs/worker/worker.js", import.meta.url), {
		type: "module",
	});

export const App: React.FC<{
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
							<RouterProvider router={router} />
							<AskpassPromptDialog />
							<Updater />
							<Toasts />
						</WorkerPoolContextProvider>
					</Tooltip.Provider>
				</Toast.Provider>
				<ReactQueryDevtools />
			</QueryClientProvider>
		</Provider>
	</StrictMode>
);
