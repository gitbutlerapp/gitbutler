<script lang="ts">
	import ComboLabelInput from '$lib/shared/ComboLabelInput.svelte';

	interface Props {
		value: string;
		disabled?: boolean;
		onChange?: (value: string) => Promise<void>;
	}

	let { value, disabled = false, onChange }: Props = $props();
</script>

<ComboLabelInput
	class="branch-description text-12"
	placeholder="Series description"
	multiline
	{value}
	{disabled}
	{onChange}
/>

<style>
	:global(.branch-description) {
		margin-bottom: 4px;
		margin-right: 13px;
	}
	:global(.branch-description::placeholder) {
		color: var(--clr-scale-ntrl-60);
	}
</style>
