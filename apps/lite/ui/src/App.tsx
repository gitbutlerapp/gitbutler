import { Toast, ToastManager, Tooltip } from "@base-ui/react";
import { useWorkerPool, WorkerPoolContextProvider } from "@pierre/diffs/react";
import { RegisteredRouter, RouterProvider } from "@tanstack/react-router";
import { type FC, StrictMode, useEffect } from "react";
import { Provider } from "react-redux";
import { store } from "#ui/store.ts";
import { Toasts } from "#ui/components/Toasts.tsx";
import { AskpassPromptDialog } from "#ui/AskpassPromptDialog.tsx";
import { useGetGUISettingsQuery } from "./api/queries.ts";
import { defaultSettings } from "./settings.ts";

const workerFactory = (): Worker =>
	new Worker(new URL("@pierre/diffs/worker/worker.js", import.meta.url), {
		type: "module",
	});

// Must be mounted under the worker pool provider.
const ThemeSync: FC = () => {
	const workerPool = useWorkerPool();
	const { data: settings } = useGetGUISettingsQuery();
	const theme = settings?.syntaxHighlighting;

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
	toastManager: ToastManager;
	router: RegisteredRouter;
}> = ({ toastManager, router }) => (
	<StrictMode>
		<Provider store={store}>
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
		</Provider>
	</StrictMode>
);
