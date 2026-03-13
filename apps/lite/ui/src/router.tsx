import { createRouter } from "@tanstack/react-router";

import { indexRoute } from "#ui/routes/index.tsx";
import { projectBranchesRoute } from "#ui/routes/project-branches.tsx";
import { projectIndexRoute } from "#ui/routes/project-index.tsx";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { rootRoute } from "#ui/routes/root.tsx";
import * as ReactQuery from "@tanstack/react-query";
import { Toast } from "@base-ui/react/toast";

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

export const toastManager = Toast.createToastManager();

export const queryClient = new ReactQuery.QueryClient({
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

const projectRouteTree = projectRootRoute.addChildren([projectIndexRoute, projectBranchesRoute]);
const routeTree = rootRoute.addChildren([indexRoute, projectRouteTree]);
export const router = createRouter({ routeTree, context: { queryClient } });

const errorMessageForToast = (error: unknown): string => {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;

	try {
		return JSON.stringify(error);
	} catch {
		return "Unknown error.";
	}
};

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}
