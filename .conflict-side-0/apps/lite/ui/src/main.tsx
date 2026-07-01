import * as ReactQuery from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import { App } from "#ui/App.tsx";
import { routeTree } from "#ui/routeTree.ts";
import { createRoot } from "react-dom/client";
import "./global.css";
import { Toast } from "@base-ui/react";
import { errorMessageForToast } from "#ui/errors.ts";

const toastManager = Toast.createToastManager();

const queryClient = new ReactQuery.QueryClient({
	defaultOptions: {
		queries: {
			// We don't expect network errors over the Node API.
			retry: false,
			staleTime: Number.POSITIVE_INFINITY,
		},
	},
});

// By default React Query uses `visibilitychange`, but this doesn't seem to work
// well in Electron.
ReactQuery.focusManager.setEventListener((setFocused) => {
	const onFocus = () => setFocused(true);
	const onBlur = () => setFocused(false);

	window.addEventListener("focus", onFocus);
	window.addEventListener("blur", onBlur);

	return () => {
		window.removeEventListener("focus", onFocus);
		window.removeEventListener("blur", onBlur);
	};
});

const router = createRouter({ routeTree, context: { queryClient } });

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Root element not found");

const root = createRoot(rootElement, {
	onUncaughtError: (error: unknown) => {
		toastManager.add({
			type: "error",
			title: "Error",
			description: errorMessageForToast(error),
			priority: "high",
		});
	},
});
root.render(<App queryClient={queryClient} toastManager={toastManager} router={router} />);
