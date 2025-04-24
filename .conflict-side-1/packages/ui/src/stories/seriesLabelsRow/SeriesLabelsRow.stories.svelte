<script module lang="ts">
	import SeriesLabelsRow from '$lib/SeriesLabelsRow.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Series Labels Row',
		component: SeriesLabelsRow,
		args: {
			series: [
				'feature/add-user-auth',
				'bugfix/fix-login-error',
				'hotfix/update-ssl-cert',
				'feature/improve-dashboard-ui',
				'release/v1.2.0',
				'feature/refactor-api-endpoints',
				'bugfix/remove-duplicate-entries',
				'chore/update-dependencies',
				'feature/add-password-reset',
				'hotfix/correct-typo-in-readme'
			],
			selected: false
		}
	});
</script>

<Story name="Playground" />
