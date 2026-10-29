import { ReduxTag, invalidatesType, providesType } from "$lib/state/tags";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { User } from "$lib/user/user";
import type { ApiUser } from "@gitbutler/shared/users/types";

export type LoginToken = {
	/** Used for polling the user; should NEVER be sent to the browser. */
	token: string;
	browser_token: string;
	expires: string;
	url: string;
};

export function buildUserEndpoints(build: BackendEndpointBuilder) {
	return {
		getUser: build.query<User | undefined, void>({
			extraOptions: { command: "get_user" },
			query: () => undefined,
			providesTags: [providesType(ReduxTag.User)],
		}),

		getUserProfile: build.query<ApiUser, void>({
			extraOptions: { command: "get_user_profile" },
			query: () => undefined,
			providesTags: [providesType(ReduxTag.UserProfile)],
		}),

		getLoginToken: build.query<LoginToken, void>({
			extraOptions: { command: "get_login_token" },
			query: () => undefined,
		}),

		setUser: build.mutation<void, { user: User }>({
			extraOptions: { command: "set_user" },
			query: (args) => args,
			invalidatesTags: [invalidatesType(ReduxTag.User)],
		}),

		deleteUser: build.mutation<void, void>({
			extraOptions: { command: "delete_user" },
			query: () => undefined,
			invalidatesTags: [invalidatesType(ReduxTag.User)],
		}),

		loginWithToken: build.mutation<User, { token: string }>({
			extraOptions: { command: "login_with_token" },
			query: (args) => args,
			invalidatesTags: [invalidatesType(ReduxTag.User)],
		}),

		updateUserProfile: build.mutation<
			any,
			{
				params: {
					name?: string;
					website?: string;
					twitter?: string;
					bluesky?: string;
					timezone?: string;
					location?: string;
					email_share?: boolean;
					avatar_base64?: string;
					avatar_filename?: string;
				};
			}
		>({
			extraOptions: { command: "update_user_profile" },
			query: (args) => args,
			invalidatesTags: [invalidatesType(ReduxTag.UserProfile)],
		}),
	};
}
