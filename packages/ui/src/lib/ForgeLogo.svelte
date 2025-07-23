<script lang="ts">
	import Icon from '$lib/Icon.svelte';

	type Props = {
		forgeName: 'github' | 'gitlab' | (string & {});
		small?: boolean;
	};

	const { forgeName, small = false }: Props = $props();

	function getLogo() {
		if (forgeName === 'gitlab') {
			if (small) {
				return 'gitlab-small';
			}

			return 'gitlab';
		} else if (forgeName === 'github') {
			if (small) {
				return 'github-small';
			}

			return 'github';
		}

		return 'question-mark-small';
	}
</script>

<Icon name={getLogo()} />
