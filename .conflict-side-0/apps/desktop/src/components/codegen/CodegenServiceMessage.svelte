<script lang="ts">
	import { ButPcAvatar } from '@gitbutler/ui';

	type Props = {
		lastUserMessageSent: Date;
	};

	const { lastUserMessageSent }: Props = $props();

	const words = [
		'contemplating',
		'reflecting',
		'reasoning',
		'analyzing',
		'pondering',
		'deliberating',
		'mulling',
		'meditating',
		'speculating',
		'conceptualizing',
		'building',
		'assembling',
		'creating',
		'developing',
		'forming',
		'fabricating',
		'composing',
		'establishing',
		'designing',
		'operating',
		'functioning',
		'executing',
		'acting',
		'butlering',
		'producing',
		'laboring',
		'performing',
		'engaging',
		'applying',
		'striving'
	];

	/**
	 * Securely generates a number between 0 and limit (inclusive)
	 */
	function randomInt(limit: number) {
		const array = new Uint32Array(1);
		crypto.getRandomValues(array);
		const randFloat = array[0]! / (0xffffffff + 1);
		return Math.round(randFloat * limit);
	}

	function getWord() {
		const i = randomInt(words.length - 1);
		return words[i];
	}

	function milisToEnglish(milis: number) {
		if (milis === 0) return 'now';

		const seconds = milis / 1000;
		const minutes = Math.floor(seconds / 60);
		const hours = Math.floor(minutes / 60);

		if (hours > 0) return `${hours}h ${minutes % 60}m`;
		if (minutes > 0) return `${minutes}m ${(seconds % 60).toFixed(1)}s`;
		return `${seconds.toFixed(1)}s`;
	}

	function getUTCNow() {
		const time = new Date();
		const offset = time.getTimezoneOffset();
		return Date.now() + offset * 1000 * 60;
	}

	let currentWord = $state(getWord());
	let currentDuration = $state(milisToEnglish(getUTCNow() - lastUserMessageSent.getTime()));

	$effect(() => {
		const updateWordInterval = setInterval(() => {
			currentWord = getWord();
		}, 1000 * 15);

		const updateTimeInterval = setInterval(() => {
			currentDuration = milisToEnglish(getUTCNow() - lastUserMessageSent.getTime());
		}, 100);

		return () => {
			clearInterval(updateWordInterval);
			clearInterval(updateTimeInterval);
		};
	});
</script>

<div class="service-message__wrapper">
	<div class="service-message">
		<ButPcAvatar mode="thinking" />
		<div class="service-message__bubble" in:popIn>
			<span class="text-13 text-italic">
				Claude is
				{#key currentWord}
					<span class="animated-word">{currentWord}</span>
				{/key}
				... {currentDuration}
			</span>
		</div>
	</div>
</div>

<style lang="postcss">
	.service-message__wrapper {
		display: flex;
		width: 100%;
		padding: 8px 0 16px;
	}
	.service-message {
		display: flex;
		align-items: flex-end;
		gap: 14px;
	}
	.service-message__bubble {
		display: flex;
		max-width: var(--message-max-width);
		padding: 10px 12px;
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.service-message__bubble--animate {
		animation: popIn 0.2s cubic-bezier(0.215, 0.61, 0.355, 1) 0.1s both;
	}

	.service-message__bubble--wiggle {
		animation:
			popIn 0.2s cubic-bezier(0.215, 0.61, 0.355, 1) 0.1s both,
			wiggle 5s ease-in-out infinite;
	}

	@keyframes popIn {
		0% {
			transform: scale(0.2) translateY(15px) rotate(-8deg);
			transform-origin: left bottom;
			opacity: 0;
		}
		100% {
			transform: scale(1) translateY(0px) rotate(0deg);
			transform-origin: left bottom;
			opacity: 1;
		}
	}

	@keyframes wiggle {
		0%,
		12%,
		100% {
			transform: translateX(0px) rotate(0deg);
		}
		2% {
			transform: translateX(-3px) rotate(-0.2deg);
		}
		4% {
			transform: translateX(3px) rotate(0.2deg);
		}
		6% {
			transform: translateX(-3px) rotate(-0.2deg);
		}
		8% {
			transform: translateX(3px) rotate(0.2deg);
		}
		10% {
			transform: translateX(0px) rotate(0deg);
		}
	}
</style>
