import type { LoadableData } from '$lib/network/types';

export type ApiUser = {
	id: number;
	login: string;
	name?: string;
	email?: string;
	avatar_url?: string;
};

export type User = {
	login: string;
	name?: string;
	email?: string;
	avatarUrl?: string;
};

export type LoadableUser = LoadableData<User, User['login']>;

export function apiToUser(apiUser: ApiUser): User {
	return {
		login: apiUser.login,
		name: apiUser.name,
		email: apiUser.email,
		avatarUrl: apiUser.avatar_url
	};
}
