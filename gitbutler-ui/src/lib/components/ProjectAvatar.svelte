<script lang="ts">
	export let name: string | undefined;

	const gradients = [
		'#E78D8D',
		'#62CDCD',
		'#EC90D2',
		'#7DC8D8',
		'#F1BC55',
		'#6B6B4C',
		'#9785DE',
		'#99CE63',
		'#636ECE',
		'#5FD2B0'
	];

	const stringToGradient = (string: string | undefined) => {
		if (!string) {
			return `linear-gradient(45deg, ${gradients[0][0]} 15%, ${gradients[0][1]} 90%)`;
		}
		// trim the string, remove all spaces
		const trimmedString = string.trim().replace(/\s/g, '');

		// this is how we take the first letter. It works with emojies.
		const startHash = trimmedString.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0);

		// Covert the hash number into the number we can
		const gradient = startHash % gradients.length;

		// and return the linear-gradient
		return gradients[gradient];
	};

	const getFirstLetter = (name: string | undefined) => {
		if (!name) return '';
		return name[0].toUpperCase();
	};

	$: firstLetter = getFirstLetter(name);
</script>

<div class="project-avatar" style:background-color={stringToGradient(name)}>
	<svg class="avatar-letter" viewBox="0 0 24 24">
		<text x="50%" y="52%" text-anchor="middle" alignment-baseline="middle">
			{firstLetter.toUpperCase()}
		</text>
	</svg>
</div>

<style>
	.project-avatar {
		flex-shrink: 0;
		width: var(--space-24);
		height: var(--space-24);
		border-radius: var(--radius-m);
	}

	.avatar-letter {
		width: 100%;
		height: 100%;
	}

	.avatar-letter text {
		font-family: 'Inter', sans-serif;
		font-weight: 800;
		font-size: 16px;
		fill: var(--clr-core-ntrl-100);
	}
</style>
