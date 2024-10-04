<script lang="ts">
	import LabelInput from '$lib/shared/LabelInput.svelte';

	interface Props {
		name: string;
		disabled?: boolean;
		onChange?: (value: string) => void;
	}

	let { name, disabled = false, onChange }: Props = $props();

	let inputEl = $state<HTMLInputElement>();
</script>

<LabelInput bind:inputEl class="text-14 text-bold" value={name} {disabled} {onChange} />
