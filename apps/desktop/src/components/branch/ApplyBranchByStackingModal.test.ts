import ApplyBranchByStackingModal from "$components/branch/ApplyBranchByStackingModal.svelte";
import { ExternallyResolvedPromise } from "$lib/utils/resolveExternally";
import { act, render, screen } from "@testing-library/svelte";
import { expect, test, vi } from "vitest";

test("prevents duplicate submissions while stacking is in progress", async () => {
	const pending = new ExternallyResolvedPromise<boolean>();
	const onStack = vi.fn(async () => await pending.promise);
	const { component } = render(ApplyBranchByStackingModal);

	await act(() =>
		component.show({
			incomingName: "incoming",
			ontoName: "destination",
			onStack,
		}),
	);
	const action = screen.getByTestId("branch-apply-stacking-modal-action-button");

	action.click();
	action.click();
	await act();

	expect(onStack).toHaveBeenCalledOnce();
	expect(action).toBeDisabled();
	pending.resolve(false);
	await pending.promise;
});

test("keeps the modal open when the retry reports that it should not close", async () => {
	const onStack = vi.fn().mockResolvedValue(false);
	const { component } = render(ApplyBranchByStackingModal);

	await act(() =>
		component.show({
			incomingName: "incoming",
			ontoName: "destination",
			onStack,
		}),
	);
	await act(() => screen.getByTestId("branch-apply-stacking-modal-action-button").click());

	expect(onStack).toHaveBeenCalledOnce();
	expect(screen.getByTestId("branch-apply-stacking-modal")).toBeVisible();
	expect(screen.getByTestId("branch-apply-stacking-modal-action-button")).not.toBeDisabled();
});
