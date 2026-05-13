import AIMacros from "$lib/ai/macros.svelte";
import { expect, test, vi } from "vitest";

function buildMacros(validateConfiguration: () => Promise<boolean>) {
	return new AIMacros(
		"project-id",
		{
			validateConfiguration,
		} as unknown as ConstructorParameters<typeof AIMacros>[1],
		{} as unknown as ConstructorParameters<typeof AIMacros>[2],
		{} as unknown as ConstructorParameters<typeof AIMacros>[3],
	);
}

test("AIMacros keeps the newest AI configuration validation result", async () => {
	let resolveFirstValidation!: (value: boolean) => void;
	let resolveSecondValidation!: (value: boolean) => void;
	const firstValidation = new Promise<boolean>((resolve) => {
		resolveFirstValidation = resolve;
	});
	const secondValidation = new Promise<boolean>((resolve) => {
		resolveSecondValidation = resolve;
	});
	const validateConfiguration = vi
		.fn()
		.mockReturnValueOnce(firstValidation)
		.mockReturnValueOnce(secondValidation);
	const macros = buildMacros(validateConfiguration);

	const firstUpdate = macros.setGenAIEnabled(true);
	const secondUpdate = macros.setGenAIEnabled(true);

	resolveSecondValidation(true);
	await secondUpdate;
	expect(macros.canUseAI).toBe(true);

	resolveFirstValidation(false);
	await firstUpdate;
	expect(macros.canUseAI).toBe(true);
});
