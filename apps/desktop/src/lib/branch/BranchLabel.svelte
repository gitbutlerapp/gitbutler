<script lang="ts">
	import ComboLabelInput from '$lib/shared/ComboLabelInput.svelte';

	interface Props {
		name: string;
		disabled?: boolean;
		readonly?: boolean;
		onChange?: (value: string) => void;
		onDblClick?: () => void;
	}

	let { name, disabled = false, onChange }: Props = $props();
</script>

<ComboLabelInput class="text-14 text-bold" value={name} {disabled} {onChange} />
