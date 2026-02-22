<script lang="ts">
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { debounce } from '$lib/utils/debounce';
	import { inject } from '@gitbutler/core/context';
	import { Icon, Textbox } from '@gitbutler/ui';

	type Props = {
		value?: string;
		helperText?: string;
		onnormalizedvalue?: (normalized: string | undefined) => void;
		onvalidationchange?: (isValid: boolean) => void;
		[key: string]: any;
	};

	let {
		value = $bindable(),
		helperText,
		onnormalizedvalue,
		onvalidationchange,
		...restProps
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);

	let textbox = $state<ReturnType<typeof Textbox>>();
	let isValidating = $state(false);
	let validationError = $state<string | undefined>();
	let validationCounter = $state(0);

	let normalizedResult = $state<{ fromValue: string; normalized: string } | undefined>();

	const isValidState = $derived(
		!isValidating && !validationError && !!value && !!normalizedResult?.normalized
	);
	$effect(() => {
		onvalidationchange?.(isValidState);
	});

	const namesDiverge = $derived(
		!!normalizedResult && normalizedResult.normalized !== normalizedResult.fromValue
	);
	const computedHelperText = $derived(
		namesDiverge && normalizedResult
			? `Will be created as '${normalizedResult.normalized}'`
			: helperText
	);

	const debouncedNormalize = debounce(async (inputValue: string) => {
		if (!inputValue) {
			isValidating = false;
			validationError = undefined;
			normalizedResult = undefined;
			onnormalizedvalue?.(undefined);
			return;
		}

		const currentValidation = ++validationCounter;
		isValidating = true;
		validationError = undefined;

		try {
			const result = await stackService.normalizeBranchName(inputValue);
			// Only update if the value hasn't changed during the async call
			// and no newer validation has started
			if (value === inputValue && currentValidation === validationCounter) {
				normalizedResult = { fromValue: inputValue, normalized: result };
				onnormalizedvalue?.(result);
				validationError = undefined;
			}
		} catch {
			if (value === inputValue && currentValidation === validationCounter) {
				normalizedResult = undefined;
				onnormalizedvalue?.(undefined);
				validationError = 'Invalid branch name';
			}
		} finally {
			if (currentValidation === validationCounter) {
				isValidating = false;
			}
		}
	}, 300);

	$effect(() => {
		debouncedNormalize(value || '');
	});

	export async function selectAll() {
		await textbox?.selectAll();
	}
</script>

<Textbox
	bind:this={textbox}
	bind:value
	helperText={computedHelperText}
	error={validationError}
	{...restProps}
>
	{#snippet customIconRight()}
		{#if isValidating}
			<Icon name="spinner" />
		{/if}
	{/snippet}
</Textbox>
