// oxlint-disable react/only-export-components -- The composition root hosts its helper components by design.
/**
 * The harness panel's composition root: builds the api over the plugin
 * transport, the app's router over a memory history (there is no URL bar),
 * and an isolated React root. Code under ui/src never learns which host it
 * is on.
 */
import { Toast, Tooltip } from "@base-ui/react";
import { useWorkerPool, WorkerPoolContextProvider } from "@pierre/diffs/react";
import { MutationCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { type FC, StrictMode, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Provider } from "react-redux";
import { createLiteApi, type LiteApiTransport } from "#electron/lite-api.ts";
import { invalidateDeclared } from "#ui/api/tags.ts";
import type { UrlQueryParams } from "#ui/cursor-url.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { createAppRouter } from "#ui/router.ts";
import { createRouteTree } from "#ui/routes.tsx";
import { defaultSettings } from "#ui/settings.ts";
import { store } from "#ui/store.ts";
import { Toasts } from "#ui/components/Toasts.tsx";
import { Panel } from "./Panel.tsx";
import type { LiteElectronApi } from "#electron/ipc.ts";

// The diff viewer's syntax-highlighting worker, bundled into the IIFE so the
// panel needs no extra asset or CSP worker source.
import DiffWorker from "@pierre/diffs/worker/worker.js?worker&inline";

import "./panel-globals.css";

/**
 * What the harness plugin drives. The bundle owns its own React 19 root
 * inside the container div, so the page's React 18 and the bundle's React
 * 19 never share a tree.
 * @public
 */
export interface PluginApp {
	mount(container: HTMLElement): void;
	update(): void;
	unmount(): void;
}

// One tree per process, as `createRouteTree` documents: it wires the shared
// route singletons together, so calling it per panel would re-parent them.
const routeTree = createRouteTree({ workspace: Panel });

const workerFactory = (): Worker => new DiffWorker();

// Must be mounted under the worker pool provider.
const SyntaxTheme: FC = () => {
	const workerPool = useWorkerPool();

	useEffect(() => {
		void workerPool?.setRenderOptions({ theme: defaultSettings.syntaxHighlighting });
	}, [workerPool]);

	return null;
};

/**
 * Entry point the harness plugin evaluates from the IIFE. The host resolves
 * the project and adapts its channel to the transport.
 * @public
 */
export default function createPanel({
	transport,
	projectId,
	params = {},
}: {
	transport: LiteApiTransport;
	projectId: string;
	params?: UrlQueryParams;
}): PluginApp {
	(window as { lite?: LiteElectronApi }).lite = createLiteApi(transport);

	const toastManager = Toast.createToastManager();

	// The same construction as the app's main.tsx: a mutation's cache effects
	// come from its endpoint's `invalidates` declaration, and its failure
	// toast from `meta.failureTitle`.
	const queryClient: QueryClient = new QueryClient({
		defaultOptions: {
			queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
		},
		mutationCache: new MutationCache({
			onSuccess: (_data, _variables, _context, mutation) =>
				invalidateDeclared(queryClient, mutation.options.mutationKey),
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

	// The app's router over a memory history, seeded at the project's
	// workspace. The shared route definitions (search validation, project
	// guard, watcher-arming loader) run here exactly as in the app.
	const searchParams = new URLSearchParams();
	for (const [key, value] of Object.entries(params))
		if (typeof value === "string") searchParams.append(key, value);
	const search = searchParams.toString();
	const router = createAppRouter(
		queryClient,
		routeTree,
		createMemoryHistory({
			initialEntries: [
				`/project/${encodeURIComponent(projectId)}/workspace${search === "" ? "" : `?${search}`}`,
			],
		}),
	);

	let root: Root | null = null;

	const render = () => {
		root?.render(
			<StrictMode>
				<Provider store={store}>
					<QueryClientProvider client={queryClient}>
						<Toast.Provider toastManager={toastManager}>
							<Tooltip.Provider>
								<WorkerPoolContextProvider
									poolOptions={{ workerFactory }}
									highlighterOptions={{ preferredHighlighter: "shiki-wasm" }}
								>
									<SyntaxTheme />
									<RouterProvider router={router} />
									<Toasts />
								</WorkerPoolContextProvider>
							</Tooltip.Provider>
						</Toast.Provider>
					</QueryClientProvider>
				</Provider>
			</StrictMode>,
		);
	};

	return {
		mount: (container) => {
			root = createRoot(container);
			render();
		},
		update: () => {
			render();
		},
		unmount: () => {
			// Closing the panel is not a navigation, so the route's onLeave never
			// runs; the binding calls watcherStopAll host-side instead.
			root?.unmount();
			root = null;
		},
	};
}
