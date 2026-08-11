import { MutationCache, QueryClient, focusManager } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import { App } from "#ui/App.tsx";
import { endpointOf, invalidateDeclared } from "#ui/api/tags.ts";
import { routeTree } from "#ui/routeTree.ts";
import { createRoot } from "react-dom/client";
import "./global.css";
import { Toast } from "@base-ui/react";
import { errorMessageForToast } from "#ui/errors.ts";

const toastManager = Toast.createToastManager();

// Annotated because the mutation-cache callback below refers back to the
// client: tsc cannot infer a type that appears in its own initializer.
const queryClient: QueryClient = new QueryClient({
	defaultOptions: {
		queries: {
			// We don't expect network errors over the Node API.
			retry: false,
			staleTime: Number.POSITIVE_INFINITY,
		},
	},
	// A mutation's cache effects come from its endpoint's `invalidates`
	// declaration, recognized by the `mutationFn` itself, and its failure
	// toast from `meta.failureTitle`; per-mutation handlers keep only
	// rollbacks, pushes, and dynamic wording.
	mutationCache: new MutationCache({
		// Returned on purpose: a mutation stays pending until the queries it
		// invalidated are fresh, so success lands together with the new data.
		onSuccess: (_data, variables, _context, mutation) =>
			invalidateDeclared(queryClient, endpointOf(mutation.options.mutationFn), variables),
		onError: (error, _variables, _context, mutation) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			const title = mutation.meta?.failureTitle;
			if (title === undefined) return;
			toastManager.add({
				type: "error",
				title,
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	}),
});

// By default React Query uses `visibilitychange`, but this doesn't seem to work
// well in Electron.
focusManager.setEventListener((setFocused) => {
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
