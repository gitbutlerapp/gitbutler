import { Toast } from "@base-ui/react/toast";
import * as ReactQuery from "@tanstack/react-query";
import { createRoot } from "react-dom/client";
import { App } from "#ui/App.tsx";
import "./global.css";

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

const errorMessageForToast = (error: unknown): string => {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;

	try {
		return JSON.stringify(error);
	} catch {
		return "Unknown error.";
	}
};

const toastManager = Toast.createToastManager();

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

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Root element not found");

const root = createRoot(rootElement);
root.render(<App queryClient={queryClient} toastManager={toastManager} />);
