import { createRouter } from "@tanstack/react-router";
import { indexRoute } from "#ui/routes/index.tsx";
import { projectRoute } from "#ui/routes/project/$id/route.tsx";
import { projectIndexRoute } from "#ui/routes/project/$id/(index)/index.tsx";
import { rootRoute } from "#ui/routes/__root.tsx";
import * as ReactQuery from "@tanstack/react-query";
import { App } from "#ui/App.tsx";
import { createRoot } from "react-dom/client";
import "./global.css";
import { Toast } from "@base-ui/react";
import { projectBranchesRoute } from "./routes/project/$id/branches/route";

const toastManager = Toast.createToastManager();

const errorMessageForToast = (error: unknown): string => {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;

	try {
		return JSON.stringify(error);
	} catch {
		return "Unknown error.";
	}
};

const queryClient = new ReactQuery.QueryClient({
	defaultOptions: {
		queries: {
			// We don't expect network errors over the Node API.
			retry: false,
		},
		mutations: {
			onError: (error: unknown) => {
				toastManager.add({
					type: "error",
					title: "Mutation failed",
					description: errorMessageForToast(error),
					priority: "high",
				});
			},
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

const routeTree = rootRoute.addChildren([
	indexRoute,
	projectRoute.addChildren([projectIndexRoute, projectBranchesRoute]),
]);
const router = createRouter({ routeTree, context: { queryClient } });

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Root element not found");

const root = createRoot(rootElement);
root.render(<App queryClient={queryClient} toastManager={toastManager} router={router} />);
