<script lang="ts">
	import { Textbox } from '@gitbutler/ui';
	import { slugify } from '@gitbutler/ui/utils/string';

	type Props = {
		value?: string;
		helperText?: string;
		onslugifiedvalue?: (slugified: string | undefined) => void;
		[key: string]: any;
	};

	let { value = $bindable(), helperText, onslugifiedvalue, ...restProps }: Props = $props();

	const slugifiedName = $derived(value && slugify(value));
	const namesDiverge = $derived(!!value && slugifiedName !== value);
	const computedHelperText = $derived(
		namesDiverge ? `Will be created as '${slugifiedName}'` : helperText
	);

	$effect(() => {
		onslugifiedvalue?.(slugifiedName);
	});
</script>

<Textbox bind:value helperText={computedHelperText} {...restProps} />
