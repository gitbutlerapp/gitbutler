import type { User } from '$lib/user/user';

export const MOCK_USER: User = {
	id: 1,
	name: 'Test User',
	email: 'testuser@example.com',
	picture: 'https://avatars.githubusercontent.com/u/35891811?v=4',
	locale: 'en-US',
	created_at: '2024-01-01T00:00:00Z',
	updated_at: '2024-01-01T00:00:00Z',
	access_token: 'mock-access-token',
	role: 'user',
	supporter: true,
	login: 'testuser'
};
