<script lang="ts">
	import Checkbox, { type CheckboxProps } from '$lib/Checkbox.svelte';

	const props: CheckboxProps = $props();
</script>

<Checkbox {...props} />
