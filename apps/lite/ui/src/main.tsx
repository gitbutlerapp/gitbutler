import { createRouter } from "@tanstack/react-router";
import { App } from "#ui/App.tsx";
import { routeTree } from "#ui/routeTree.ts";
import { createRoot } from "react-dom/client";
import "./global.css";
import { Toast } from "@base-ui/react";
import { errorMessageForToast } from "#ui/errors.ts";

const toastManager = Toast.createToastManager();

const router = createRouter({ routeTree });

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
root.render(<App toastManager={toastManager} router={router} />);
