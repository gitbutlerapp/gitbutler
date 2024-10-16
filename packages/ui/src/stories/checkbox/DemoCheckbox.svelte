<script lang="ts">
	import Checkbox, { type Props as CheckboxProps } from '$lib/Checkbox.svelte';

	const props: CheckboxProps = $props();
</script>

<Checkbox {...props} />
