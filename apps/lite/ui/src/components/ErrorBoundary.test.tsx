/** @vitest-environment jsdom */

import { act, type FC, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ErrorBoundary } from "./ErrorBoundary.tsx";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const Boom: FC<{ message?: string }> = ({ message = "boom" }) => {
	throw new Error(message);
};

const Fine: FC<{ label: string }> = ({ label }) => <p>{label}</p>;

describe("ErrorBoundary", () => {
	let container: HTMLDivElement;
	let root: Root;

	beforeEach(() => {
		// React logs caught errors, and the boundary logs them again on purpose.
		vi.spyOn(console, "error").mockImplementation(() => {});
		container = document.createElement("div");
		document.body.append(container);
		root = createRoot(container);
	});

	afterEach(() => {
		act(() => root.unmount());
		container.remove();
		vi.restoreAllMocks();
	});

	const render = (children: ReactNode, resetKeys?: ReadonlyArray<unknown>) => {
		act(() =>
			root.render(
				<div>
					<p>sibling</p>
					<ErrorBoundary resetKeys={resetKeys}>{children}</ErrorBoundary>
				</div>,
			),
		);
	};

	it("keeps a failing subtree from taking its siblings with it", () => {
		render(<Boom message="details exploded" />);

		expect(container.textContent).toContain("sibling");
		expect(container.textContent).toContain("Something went wrong.");
		expect(container.querySelector("code")?.textContent).toBe("details exploded");
	});

	it("renders children again once a reset key changes", () => {
		const failing = <Boom />;
		render(failing, [failing]);
		expect(container.textContent).toContain("Something went wrong.");

		const working = <Fine label="recovered" />;
		render(working, [working]);

		expect(container.textContent).toContain("recovered");
		expect(container.textContent).not.toContain("Something went wrong.");
	});

	it("stays failed while the reset key holds, so a rerender cannot loop it", () => {
		const failing = <Boom />;
		render(failing, [failing]);
		render(failing, [failing]);

		expect(container.textContent).toContain("Something went wrong.");
	});

	it("stays failed with no reset keys until Retry is pressed", () => {
		let child: ReactNode = <Boom />;
		render(child);
		expect(container.textContent).toContain("Something went wrong.");

		// A plain rerender is not enough without keys...
		child = <Fine label="recovered" />;
		render(child);
		expect(container.textContent).toContain("Something went wrong.");

		// ...but Retry clears it, and the current children render.
		const retry = [...container.querySelectorAll("button")].find(
			(button) => button.textContent === "Retry",
		);
		act(() => retry?.click());

		expect(container.textContent).toContain("recovered");
	});

	it("calls onReset when Retry is pressed", () => {
		const onReset = vi.fn();
		act(() =>
			root.render(
				<ErrorBoundary onReset={onReset}>
					<Boom />
				</ErrorBoundary>,
			),
		);

		const retry = [...container.querySelectorAll("button")].find(
			(button) => button.textContent === "Retry",
		);
		act(() => retry?.click());

		expect(onReset).toHaveBeenCalledOnce();
	});
});
