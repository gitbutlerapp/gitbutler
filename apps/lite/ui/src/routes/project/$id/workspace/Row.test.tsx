/** @vitest-environment jsdom */

import { flushSync } from "react-dom";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Row } from "./Row.tsx";

describe("Row", () => {
	let container: HTMLDivElement;
	let root: Root;

	beforeEach(() => {
		container = document.createElement("div");
		document.body.append(container);
		root = createRoot(container);
	});

	afterEach(() => {
		root.unmount();
		container.remove();
	});

	it("uses Shift-click for a row-body action without changing descendant behavior", () => {
		const onSelect = vi.fn();
		const onShiftSelect = vi.fn();
		flushSync(() =>
			root.render(
				<Row onSelect={onSelect} onShiftSelect={onShiftSelect}>
					<span data-body>Body</span>
					<a href="#target">Link</a>
					<button type="button">Button</button>
					<input type="checkbox" />
				</Row>,
			),
		);

		container
			.querySelector("[data-body]")
			?.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
		expect(onShiftSelect).toHaveBeenCalledOnce();
		expect(onSelect).not.toHaveBeenCalled();

		container
			.querySelector("[data-body]")
			?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
		expect(onSelect).toHaveBeenCalledOnce();

		container
			.querySelector("a")
			?.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
		expect(onShiftSelect).toHaveBeenCalledOnce();
		expect(onSelect).toHaveBeenCalledTimes(2);

		container
			.querySelector("button")
			?.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
		container
			.querySelector("input")
			?.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
		expect(onShiftSelect).toHaveBeenCalledOnce();
		expect(onSelect).toHaveBeenCalledTimes(2);
	});

	it("keeps Shift-click as selection when no Shift action is available", () => {
		const onSelect = vi.fn();
		flushSync(() =>
			root.render(
				<Row onSelect={onSelect}>
					<span data-body>Body</span>
				</Row>,
			),
		);

		container
			.querySelector("[data-body]")
			?.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
		expect(onSelect).toHaveBeenCalledOnce();
	});
});
