import type { LoadableData } from '$lib/network/types';

export type ApiUserSimple = {
	id: number;
	avatar_url: string | null;
	email: string | null;
	login: string | null;
	name: string | null;
};

export type UserSimple = {
	id: number;
	avatarUrl: string | undefined;
	email: string | undefined;
	login: string | undefined;
	name: string | undefined;
};

export function apiToUserSimple(api: ApiUserSimple): UserSimple {
	return {
		id: api.id,
		avatarUrl: api.avatar_url ?? undefined,
		email: api.email ?? undefined,
		login: api.login ?? undefined,
		name: api.name ?? undefined
	};
}

export type ApiUser = ApiUserSimple & {
	given_name?: string;
	family_name?: string;
	picture?: string;
	locale?: string;
	access_token?: string;
	updated_at: string;
	created_at: string;
	supporter?: boolean;
	role?: string;
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
		...apiToUserSimple(apiUser),
		login: apiUser.login ?? '' // Shouldn't be null, but we need to make sure
	};
}

export type ApiUserPrivate = ApiUser & {
	access_token: string;
	role: string | null;
};

export type UserPrivate = {
	accessToken: string;
	role: string | undefined;
};

export function apiToUserPrivate(api: ApiUserPrivate): UserPrivate {
	return {
		...apiToUser(api),
		accessToken: api.access_token,
		role: api.role ?? undefined
	};
}
