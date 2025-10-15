import type { AuthenticatedUser } from '$lib/forge/github/githubUserService.svelte';
export const MOCK_AUTH_USER: AuthenticatedUser = {
	accessToken: 'mock-access-token',
	login: 'mockuser',
	name: 'Mock User',
	email: 'bla@user.com',
	avatarUrl: 'https://avatars.githubusercontent.com/u/35891811?v=4'
};
