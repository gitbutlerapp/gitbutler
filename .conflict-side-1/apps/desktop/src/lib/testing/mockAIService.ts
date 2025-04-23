import { vi } from 'vitest';

export function getAIServiceMock() {
	const AIServiceMock = vi.fn();

	AIServiceMock.prototype.validateConfiguration = vi.fn(async () => {
		return await Promise.resolve(true);
	});

	return AIServiceMock;
}
