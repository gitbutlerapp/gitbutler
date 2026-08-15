/** @vitest-environment jsdom */

import { act, Suspense, type FC, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	QueryClient,
	QueryClientProvider,
	QueryErrorResetBoundary,
	useSuspenseQuery,
} from "@tanstack/react-query";

import { ErrorBoundary } from "./ErrorBoundary.tsx";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

let shouldFail = true;

const Data: FC = () => {
	const { data } = useSuspenseQuery({
		// Keys are branded to the app's own set (see api/queries.ts), so this
		// borrows a real one; the client below is this test's alone.
		queryKey: ["reviewedFiles"],
		queryFn: async () => {
			if (shouldFail) throw new Error("query failed");
			return "loaded";
		},
	});
	return <p>{data}</p>;
};

describe("ErrorBoundary with a suspense query", () => {
	let container: HTMLDivElement;
	let root: Root;
	let queryClient: QueryClient;

	beforeEach(() => {
		vi.spyOn(console, "error").mockImplementation(() => {});
		shouldFail = true;
		// Mirrors main.tsx.
		queryClient = new QueryClient({
			defaultOptions: { queries: { retry: false, staleTime: Number.POSITIVE_INFINITY } },
		});
		container = document.createElement("div");
		document.body.append(container);
		root = createRoot(container);
	});

	afterEach(() => {
		act(() => root.unmount());
		container.remove();
		vi.restoreAllMocks();
	});

	// Mirrors how WorkspacePage wires its panes: the boundary always gets the
	// query reset, which is what lets a failed suspense query be retried.
	const render = (resetKeys: ReadonlyArray<unknown>) => {
		const tree: ReactNode = (
			<QueryClientProvider client={queryClient}>
				<QueryErrorResetBoundary>
					{({ reset }) => (
						<ErrorBoundary resetKeys={resetKeys} onReset={reset}>
							<Suspense fallback={<p>loading</p>}>
								<Data />
							</Suspense>
						</ErrorBoundary>
					)}
				</QueryErrorResetBoundary>
			</QueryClientProvider>
		);
		act(() => root.render(tree));
	};

	const settle = async () => {
		await act(async () => {
			await new Promise((resolve) => setTimeout(resolve, 0));
		});
	};

	it("shows the failure", async () => {
		render(["a"]);
		await settle();
		expect(container.textContent).toContain("Something went wrong.");
	});

	it("recovers on a reset key once the query would succeed", async () => {
		render(["a"]);
		await settle();
		expect(container.textContent).toContain("Something went wrong.");

		shouldFail = false;
		render(["b"]);
		await settle();
		await settle();

		expect(container.textContent).toContain("loaded");
	});

	it("recovers on Retry once the query would succeed", async () => {
		render(["a"]);
		await settle();
		expect(container.textContent).toContain("Something went wrong.");

		shouldFail = false;
		const retry = [...container.querySelectorAll("button")].find(
			(button) => button.textContent === "Retry",
		);
		act(() => retry?.click());
		await settle();
		await settle();

		expect(container.textContent).toContain("loaded");
	});
});
