import confetti from 'canvas-confetti';

export function sprayConfetti(event: MouseEvent) {
	const x = event.clientX / window.innerWidth;
	const y = event.clientY / window.innerHeight;
	confetti({
		particleCount: 50,
		spread: 70,
		origin: { x, y },
		startVelocity: 25
	});
}
