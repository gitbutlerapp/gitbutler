import confetti from 'canvas-confetti';

export function sprayConfetti(event: MouseEvent) {
	const x = event.clientX / window.innerWidth;
	const y = event.clientY / window.innerHeight;
	confetti({
		particleCount: 30,
		spread: 360,
		gravity: 0.2,
		origin: { x, y },
		startVelocity: 15,
		ticks: 60,
		colors: ['#7FE9E4', '#FF8080', '#8280FF', '#FFD874']
	});
}
